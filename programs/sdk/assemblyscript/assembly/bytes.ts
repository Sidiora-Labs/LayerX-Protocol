/**
 * Integer-only byte primitives.
 *
 * The guest boundary carries integers and linear-memory offsets, nothing else.
 * Every conversion in this module is exact: big-endian reads and writes, byte
 * comparison, and the four-word form a thirty-two byte identifier takes when it
 * crosses an export signature. No routine here touches a floating-point value,
 * a clock, or a source of entropy.
 */

export const IDENTIFIER_BYTES: i32 = 32;
export const AMOUNT_BYTES: i32 = 16;

/** Returns the linear-memory offset of a caller-owned byte array. */
export function pointer(value: StaticArray<u8>): i32 {
  return <i32>changetype<usize>(value);
}

/** Allocates a zeroed byte array of the declared length. */
export function allocate(length: i32): StaticArray<u8> {
  return new StaticArray<u8>(length);
}

/** Writes an unsigned sixteen-bit integer in network order. */
export function writeU16BE(out: StaticArray<u8>, offset: i32, value: u16): void {
  const wide = <u32>value;
  out[offset] = <u8>(wide >> 8);
  out[offset + 1] = <u8>wide;
}

/** Writes an unsigned thirty-two bit integer in network order. */
export function writeU32BE(out: StaticArray<u8>, offset: i32, value: u32): void {
  out[offset] = <u8>(value >> 24);
  out[offset + 1] = <u8>(value >> 16);
  out[offset + 2] = <u8>(value >> 8);
  out[offset + 3] = <u8>value;
}

/** Writes an unsigned sixty-four bit integer in network order. */
export function writeU64BE(out: StaticArray<u8>, offset: i32, value: u64): void {
  for (let index = 0; index < 8; index++) {
    const shift = <u64>(56 - index * 8);
    out[offset + index] = <u8>(value >> shift);
  }
}

/** Reads an unsigned sixteen-bit integer written in network order. */
export function readU16BE(source: StaticArray<u8>, offset: i32): u16 {
  const high = <u32>source[offset];
  const low = <u32>source[offset + 1];
  return <u16>((high << 8) | low);
}

/** Reads an unsigned thirty-two bit integer written in network order. */
export function readU32BE(source: StaticArray<u8>, offset: i32): u32 {
  const b0 = <u32>source[offset];
  const b1 = <u32>source[offset + 1];
  const b2 = <u32>source[offset + 2];
  const b3 = <u32>source[offset + 3];
  return (b0 << 24) | (b1 << 16) | (b2 << 8) | b3;
}

/** Reads an unsigned sixty-four bit integer written in network order. */
export function readU64BE(source: StaticArray<u8>, offset: i32): u64 {
  let value: u64 = 0;
  for (let index = 0; index < 8; index++) {
    value = (value << 8) | <u64>source[offset + index];
  }
  return value;
}

/** Reads the two's-complement thirty-two bit integer a receipt view carries. */
export function readI32BE(source: StaticArray<u8>, offset: i32): i32 {
  return <i32>readU32BE(source, offset);
}

/** Copies a run of bytes between two caller-owned arrays. */
export function copy(
  destination: StaticArray<u8>,
  destinationOffset: i32,
  source: StaticArray<u8>,
  sourceOffset: i32,
  length: i32
): void {
  for (let index = 0; index < length; index++) {
    destination[destinationOffset + index] = source[sourceOffset + index];
  }
}

/** Orders two byte runs lexicographically. */
export function compare(
  left: StaticArray<u8>,
  leftOffset: i32,
  right: StaticArray<u8>,
  rightOffset: i32,
  length: i32
): i32 {
  for (let index = 0; index < length; index++) {
    const first = left[leftOffset + index];
    const second = right[rightOffset + index];
    if (first != second) return first < second ? -1 : 1;
  }
  return 0;
}

/** Reports whether two byte runs hold the same bytes. */
export function equal(
  left: StaticArray<u8>,
  leftOffset: i32,
  right: StaticArray<u8>,
  rightOffset: i32,
  length: i32
): bool {
  return compare(left, leftOffset, right, rightOffset, length) == 0;
}

/** Reports whether every byte is the zero the protocol reserves for absence. */
export function isZero(value: StaticArray<u8>): bool {
  for (let index = 0; index < value.length; index++) {
    if (value[index] != 0) return false;
  }
  return true;
}

/** Copies a slice out of a caller-owned array. */
export function slice(source: StaticArray<u8>, offset: i32, length: i32): StaticArray<u8> {
  const output = new StaticArray<u8>(length);
  copy(output, 0, source, offset, length);
  return output;
}

/**
 * Rebuilds a thirty-two byte identifier from the four big-endian words an
 * export signature carries, because the guest boundary admits integers only.
 */
export function identifierFromWords(word0: u64, word1: u64, word2: u64, word3: u64): StaticArray<u8> {
  const value = new StaticArray<u8>(IDENTIFIER_BYTES);
  writeU64BE(value, 0, word0);
  writeU64BE(value, 8, word1);
  writeU64BE(value, 16, word2);
  writeU64BE(value, 24, word3);
  return value;
}

/** Encodes a source-level constant as its exact UTF-8 bytes. */
export function fromString(value: string): StaticArray<u8> {
  const encoded = String.UTF8.encode(value);
  const length = encoded.byteLength;
  const output = new StaticArray<u8>(length);
  memory.copy(changetype<usize>(output), changetype<usize>(encoded), <usize>length);
  return output;
}
