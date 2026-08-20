/**
 * Nonzero protocol identifiers.
 *
 * Every identifier the ABI carries is thirty-two bytes wide and reserves the
 * all-zero value for absence. Each binding that hands an identifier to the host
 * refuses the reserved value first, so a program cannot name a program, an
 * asset, an account or a receipt the runtime would have to refuse.
 */

import { IDENTIFIER_BYTES, identifierFromWords, isZero, slice } from "./bytes";

/** Stable identifier of a deployed program. */
export class ProgramId {
  bytes: StaticArray<u8>;

  constructor(bytes: StaticArray<u8>) {
    this.bytes = bytes;
  }

  /** Rebuilds a program identifier from four big-endian words. */
  static fromWords(word0: u64, word1: u64, word2: u64, word3: u64): ProgramId {
    return new ProgramId(identifierFromWords(word0, word1, word2, word3));
  }

  /** Copies a program identifier out of a caller-owned array. */
  static fromBytes(source: StaticArray<u8>, offset: i32): ProgramId {
    return new ProgramId(slice(source, offset, IDENTIFIER_BYTES));
  }

  /** Reports whether this is the all-zero identifier reserved for absence. */
  isReserved(): bool {
    return isZero(this.bytes);
  }
}

/** Stable identifier of a protocol asset. */
export class AssetId {
  bytes: StaticArray<u8>;

  constructor(bytes: StaticArray<u8>) {
    this.bytes = bytes;
  }

  /** Rebuilds an asset identifier from four big-endian words. */
  static fromWords(word0: u64, word1: u64, word2: u64, word3: u64): AssetId {
    return new AssetId(identifierFromWords(word0, word1, word2, word3));
  }

  /** Copies an asset identifier out of a caller-owned array. */
  static fromBytes(source: StaticArray<u8>, offset: i32): AssetId {
    return new AssetId(slice(source, offset, IDENTIFIER_BYTES));
  }

  /** Reports whether this is the all-zero identifier reserved for absence. */
  isReserved(): bool {
    return isZero(this.bytes);
  }
}

/** Stable identifier of an account a 402LXP transfer may credit. */
export class AccountId {
  bytes: StaticArray<u8>;

  constructor(bytes: StaticArray<u8>) {
    this.bytes = bytes;
  }

  /** Rebuilds an account identifier from four big-endian words. */
  static fromWords(word0: u64, word1: u64, word2: u64, word3: u64): AccountId {
    return new AccountId(identifierFromWords(word0, word1, word2, word3));
  }

  /** Copies an account identifier out of a caller-owned array. */
  static fromBytes(source: StaticArray<u8>, offset: i32): AccountId {
    return new AccountId(slice(source, offset, IDENTIFIER_BYTES));
  }

  /** Reports whether this is the all-zero identifier reserved for absence. */
  isReserved(): bool {
    return isZero(this.bytes);
  }
}

/** Digest naming one verified protocol receipt. */
export class ReceiptDigest {
  bytes: StaticArray<u8>;

  constructor(bytes: StaticArray<u8>) {
    this.bytes = bytes;
  }

  /** Rebuilds a receipt digest from four big-endian words. */
  static fromWords(word0: u64, word1: u64, word2: u64, word3: u64): ReceiptDigest {
    return new ReceiptDigest(identifierFromWords(word0, word1, word2, word3));
  }

  /** Copies a receipt digest out of a caller-owned array. */
  static fromBytes(source: StaticArray<u8>, offset: i32): ReceiptDigest {
    return new ReceiptDigest(slice(source, offset, IDENTIFIER_BYTES));
  }

  /** Reports whether this is the all-zero digest reserved for absence. */
  isReserved(): bool {
    return isZero(this.bytes);
  }
}
