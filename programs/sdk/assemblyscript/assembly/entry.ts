/**
 * Entrypoint plumbing owned by the SDK.
 *
 * The composition layer enters a callee by asking it to reserve a bounded region
 * of its own linear memory, writing the call input there, and then invoking the
 * call entry export. The reservation lives here, is never resized, and always
 * hands out the same address, so a caller cannot steer a program at an offset of
 * its own choosing and a program declares its entry points without owning a raw
 * pointer of its own.
 */

import { CALL_INPUT_CAPACITY, RESERVATION_REFUSED } from "./abi";
import { pointer, slice } from "./bytes";
import { ERR_INPUT_TOO_LARGE, ERR_INVALID } from "./error";

const CALL_INPUT: StaticArray<u8> = new StaticArray<u8>(CALL_INPUT_CAPACITY);

/**
 * Reserves the call-input region for a caller of the declared length, returning
 * its address or a negative refusal.
 */
export function reserveCallInput(length: i32): i32 {
  if (length < 0) return RESERVATION_REFUSED;
  if (length > CALL_INPUT_CAPACITY) return RESERVATION_REFUSED;
  return pointer(CALL_INPUT);
}

/**
 * Admits the call input the caller wrote into the reserved region, returning its
 * length or a negative refusal. A length past the declared reservation and any
 * pointer other than the one `reserveCallInput` handed out are refused.
 */
export function acceptCallInput(inputPointer: i32, inputLength: i32): i32 {
  if (inputLength < 0) return ERR_INVALID;
  if (inputLength > CALL_INPUT_CAPACITY) return ERR_INPUT_TOO_LARGE;
  if (inputLength > 0 && inputPointer != pointer(CALL_INPUT)) return ERR_INVALID;
  return inputLength;
}

/** Borrows the reserved call-input region. */
export function callInputRegion(): StaticArray<u8> {
  return CALL_INPUT;
}

/** Copies the admitted call input out of the reserved region. */
export function callInputBytes(length: i32): StaticArray<u8> {
  return slice(CALL_INPUT, 0, length);
}
