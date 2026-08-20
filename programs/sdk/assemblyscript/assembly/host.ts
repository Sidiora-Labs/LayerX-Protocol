/**
 * Raw `layerx_v1` imports.
 *
 * These seven declarations are the whole host surface a version-one program may
 * import. The deterministic-subset validator refuses any module importing
 * anything else, so a clock, a socket, a thread or a source of entropy cannot be
 * reached even by accident, and this module is the only place guest code crosses
 * into the runtime.
 */

// @ts-ignore: decorator
@external("layerx_v1", "storage_read")
export declare function storageRead(
  keyPointer: i32,
  keyLength: i32,
  outputPointer: i32,
  outputCapacity: i32
): i32;

// @ts-ignore: decorator
@external("layerx_v1", "storage_write")
export declare function storageWrite(
  keyPointer: i32,
  keyLength: i32,
  valuePointer: i32,
  valueLength: i32
): i32;

// @ts-ignore: decorator
@external("layerx_v1", "storage_delete")
export declare function storageDelete(keyPointer: i32, keyLength: i32): i32;

// @ts-ignore: decorator
@external("layerx_v1", "event_emit")
export declare function eventEmit(
  topicPointer: i32,
  topicLength: i32,
  dataPointer: i32,
  dataLength: i32
): i32;

// @ts-ignore: decorator
@external("layerx_v1", "program_call")
export declare function programCall(
  programPointer: i32,
  programLength: i32,
  inputPointer: i32,
  inputLength: i32,
  capabilitiesPointer: i32,
  capabilitiesLength: i32
): i32;

// @ts-ignore: decorator
@external("layerx_v1", "transfer_402")
export declare function transfer402(
  amountHigh: i64,
  amountLow: i64,
  assetPointer: i32,
  assetLength: i32,
  recipientPointer: i32,
  recipientLength: i32
): i32;

// @ts-ignore: decorator
@external("layerx_v1", "receipt_read")
export declare function receiptRead(
  digestPointer: i32,
  digestLength: i32,
  outputPointer: i32,
  outputCapacity: i32
): i32;
