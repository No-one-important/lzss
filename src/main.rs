use std::env;
use std::fs;
use std::fs::File;
use std::io::prelude::*;

const MAX_DIST: usize = 32767;
const MAX_LEN: usize = 32767;

// min len of six bytes needed to save space
struct BackRef {
    dist: u16,
    len: u16,
}

struct LitData {
    data: Vec<u8>,
}

enum Chunk {
    Data(LitData),
    BackRef(BackRef),
}

impl Chunk {
    fn bytes(self: &Self) -> Vec<u8> {
        let mut output = vec![];
        match self {
            Chunk::Data(data) => {
                let len = data.data.len();
                if len < 64 {
                    output.push(0b0011_1111 & len as u8);
                } else {
                    output.push(0b0100_0000);

                    output.push((len >> 8) as u8);
                    output.push(len as u8);
                }
                for b in &data.data {
                    output.push(*b);
                }
            }
            Chunk::BackRef(back_ref) => {
                let len = back_ref.len;
                let dist = back_ref.dist;
                // println!("back ref dist: {dist}");

                if len < 64 {
                    output.push(0b1000_0000_u8 | len as u8);
                } else {
                    output.push(0b1100_0000);

                    output.push((len >> 8) as u8);
                    output.push(len as u8);
                }

                if dist < 128 {
                    output.push(0b0000_0000_u8 | dist as u8);
                } else {
                    output.push(0b1000_0000 | (dist >> 8) as u8);
                    output.push(dist as u8);
                }
            }
        }

        output
    }
}

fn main() {
    let fname = env::args().nth(1).unwrap();
    let data = fs::read(fname).unwrap();
    dbg!(blake3::hash(&data));
    let comped = compress(&data);
    dbg!(data.len());

    let mut f = File::create("compressed").unwrap();
    f.write_all(&comped);
    dbg!(comped.len());

    let mut f = File::create("decompressed").unwrap();
    let decompressed = decompress(comped);
    dbg!(blake3::hash(&decompressed));
    dbg!(decompressed.len());
    f.write_all(&decompressed);
}

fn compress(input: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(input.len() / 2);

    let mut current_chunk = LitData {
        data: Vec::with_capacity(64),
    };

    current_chunk.data.push(input[0]);

    let mut i = 1;
    // loop over input
    while i < input.len() {
        let mut max_len = 0;
        let mut best_dist = 0;
        let start_index = if i > MAX_DIST { i - MAX_DIST } else { 0 };
        let win_len = if i > MAX_LEN { MAX_LEN } else { i };

        // finding backrefs
        for j in 0..win_len {
            let mut len = 0;

            while input[start_index + j + len] == input[i + len] && len < MAX_LEN {
                len += 1;
                if i + len >= input.len() {
                    break;
                }
            }

            if len > max_len {
                max_len = len;
                best_dist = i - (start_index + j);
            }
        }

        if max_len > 6 {
            i += max_len - 1;
            if current_chunk.data.len() != 0 {
                output.extend_from_slice(&Chunk::Data(current_chunk).bytes());
            }
            output.extend_from_slice(&Chunk::BackRef(BackRef {
                len: max_len as u16,
                dist: best_dist as u16,
            })
            .bytes());

            current_chunk = LitData { data: vec![] };
        } else {
            current_chunk.data.push(input[i]);
        }

        if current_chunk.data.len() > 65535 {
            output.extend_from_slice(&Chunk::Data(current_chunk).bytes());

            current_chunk = LitData { data: vec![] };
        }

        i += 1;
    }

    if current_chunk.data.len() != 0 {
        for b in Chunk::Data(current_chunk).bytes() {
            output.push(b);
        }
    }

    output
}

fn decompress(mut input: Vec<u8>) -> Vec<u8> {
    let mut output = vec![];

    loop {
        if input[0] & 0b1000_0000 != 0 {
            // backref
            let len: usize;

            if input[0] & 0b0100_0000 != 0 {
                // multi byte
                len = ((input[1] as usize) << 8) | input[2] as usize;
                input.drain(0..3);
            } else {
                len = (input[0] as usize) & 0b0011_1111;
                input.drain(0..1);
            }

            let dist: usize;

            if input[0] & 0b1000_0000 != 0 {
                // multi byte
                dist = ((input[0] as usize & 0b0111_1111) << 8) | input[1] as usize;
                input.drain(0..2);
            } else {
                dist = (input[0] as usize & 0b0111_1111);
                input.drain(0..1);
            }

            let out_len = output.len();
            for i in 0..len {
                output.push(output[(out_len - dist) + i]);
            }
        } else {
            // data
            let len: usize;

            if input[0] & 0b0100_0000 != 0 {
                // multi byte
                len = ((input[1] as usize) << 8) | input[2] as usize;
                input.drain(0..3);
            } else {
                len = (input[0] as usize) & 0b0011_1111;
                input.drain(0..1);
            }

            for i in 0..len {
                output.push(input[i]);
            }
            input.drain(0..len);
        }

        if input.is_empty() {
            break;
        }
    }

    output
}
