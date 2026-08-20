//! `Keccak-256` exactly as the EVM computes it, so ported slot addresses,
//! event topics and method selectors are byte-identical to the values an
//! existing Solidity deployment already uses.

const ROUNDS: usize = 24;
const RATE: usize = 136;

const ROUND_CONSTANTS: [u64; ROUNDS] = [
    0x0000_0000_0000_0001,
    0x0000_0000_0000_8082,
    0x8000_0000_0000_808a,
    0x8000_0000_8000_8000,
    0x0000_0000_0000_808b,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8009,
    0x0000_0000_0000_008a,
    0x0000_0000_0000_0088,
    0x0000_0000_8000_8009,
    0x0000_0000_8000_000a,
    0x0000_0000_8000_808b,
    0x8000_0000_0000_008b,
    0x8000_0000_0000_8089,
    0x8000_0000_0000_8003,
    0x8000_0000_0000_8002,
    0x8000_0000_0000_0080,
    0x0000_0000_0000_800a,
    0x8000_0000_8000_000a,
    0x8000_0000_8000_8081,
    0x8000_0000_0000_8080,
    0x0000_0000_8000_0001,
    0x8000_0000_8000_8008,
];

/// Computes the `Keccak-256` digest of `input`.
#[must_use]
pub fn keccak256(input: &[u8]) -> [u8; 32] {
    let mut state = [0u64; 25];
    let mut offset = 0usize;
    while offset.saturating_add(RATE) <= input.len() {
        let end = offset.saturating_add(RATE);
        absorb(&mut state, &input[offset..end]);
        permute(&mut state);
        offset = end;
    }
    let tail = &input[offset..];
    let mut block = [0u8; RATE];
    block[..tail.len()].copy_from_slice(tail);
    block[tail.len()] = 0x01;
    block[RATE - 1] |= 0x80;
    absorb(&mut state, &block);
    permute(&mut state);
    let mut digest = [0u8; 32];
    for (index, chunk) in digest.chunks_exact_mut(8).enumerate() {
        chunk.copy_from_slice(&state[index].to_le_bytes());
    }
    digest
}

/// Computes the four-byte method selector of a canonical signature.
#[must_use]
pub fn selector(signature: &str) -> [u8; 4] {
    let digest = keccak256(signature.as_bytes());
    [digest[0], digest[1], digest[2], digest[3]]
}

fn absorb(state: &mut [u64; 25], block: &[u8]) {
    for (index, chunk) in block.chunks_exact(8).enumerate() {
        let mut lane = [0u8; 8];
        lane.copy_from_slice(chunk);
        state[index] ^= u64::from_le_bytes(lane);
    }
}

fn permute(state: &mut [u64; 25]) {
    for constant in ROUND_CONSTANTS {
        let mut column = [0u64; 5];
        for (index, lane) in column.iter_mut().enumerate() {
            *lane = state[index]
                ^ state[index + 5]
                ^ state[index + 10]
                ^ state[index + 15]
                ^ state[index + 20];
        }
        let mut theta = [0u64; 5];
        for (index, lane) in theta.iter_mut().enumerate() {
            *lane = column[(index + 4) % 5] ^ column[(index + 1) % 5].rotate_left(1);
        }
        for row in 0..5usize {
            for (index, lane) in theta.iter().enumerate() {
                state[index + 5 * row] ^= *lane;
            }
        }
        let mut x = 1usize;
        let mut y = 0usize;
        let mut current = state[1];
        for step in 0..24u32 {
            let next_x = y;
            let next_y = (2 * x + 3 * y) % 5;
            let index = next_x + 5 * next_y;
            let carried = state[index];
            let shift = ((step + 1) * (step + 2) / 2) % 64;
            state[index] = current.rotate_left(shift);
            current = carried;
            x = next_x;
            y = next_y;
        }
        for row in 0..5usize {
            let base = 5 * row;
            let lanes = [
                state[base],
                state[base + 1],
                state[base + 2],
                state[base + 3],
                state[base + 4],
            ];
            for (index, lane) in lanes.iter().enumerate() {
                state[base + index] = *lane ^ (!lanes[(index + 1) % 5] & lanes[(index + 2) % 5]);
            }
        }
        state[0] ^= constant;
    }
}
