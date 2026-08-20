/**
 * Guest-side refusal taxonomy.
 *
 * Status numbers cross the guest boundary inside canonical execution evidence
 * and are consensus data. The first band mirrors the host status codes exactly;
 * the second band is guest-side refusal the host never produces. Renumbering
 * any value is a protocol-version change, never a refactor. The same numbers
 * are declared by every LayerX authoring language.
 */

export const OK: i32 = 0;
export const ERR_DENIED: i32 = -1;
export const ERR_INVALID: i32 = -2;
export const ERR_BOUNDS: i32 = -3;
export const ERR_METER: i32 = -4;
export const ERR_EVIDENCE: i32 = -5;
export const ERR_NULL_ARGUMENT: i32 = -16;
export const ERR_EMPTY_KEY: i32 = -17;
export const ERR_KEY_TOO_LARGE: i32 = -18;
export const ERR_VALUE_TOO_LARGE: i32 = -19;
export const ERR_EMPTY_TOPIC: i32 = -20;
export const ERR_TOPIC_TOO_LARGE: i32 = -21;
export const ERR_DATA_TOO_LARGE: i32 = -22;
export const ERR_INPUT_TOO_LARGE: i32 = -23;
export const ERR_ZERO_AMOUNT: i32 = -24;
export const ERR_RESERVED_IDENTIFIER: i32 = -25;
export const ERR_DUPLICATE_CAPABILITY: i32 = -26;
export const ERR_CAPABILITY_LIMIT: i32 = -27;
export const ERR_CAPABILITY_BYTES: i32 = -28;
export const ERR_BUFFER_TOO_SMALL: i32 = -29;
export const ERR_RECEIPT_ENCODING: i32 = -30;
export const ERR_OVERFLOW: i32 = -31;
export const ERR_UNDERFLOW: i32 = -32;

/** Reports whether a status names a refusal rather than a result. */
export function isRefusal(status: i32): bool {
  return status < 0;
}

/** Reports whether a refusal was produced by the host rather than the guest. */
export function isHostRefusal(status: i32): bool {
  return status < 0 && status >= ERR_EVIDENCE;
}

/**
 * Collapses a guest-side refusal onto the frozen host status band so the
 * integer an entrypoint returns is the same in every authoring language. A
 * bound, a capacity or an exact-integer overflow becomes ERR_BOUNDS; a reserved
 * value, a duplicate authority or a non-canonical encoding becomes ERR_INVALID.
 */
export function abiStatus(status: i32): i32 {
  if (status >= 0) return status;
  if (status >= ERR_EVIDENCE) return status;
  if (
    status == ERR_EMPTY_KEY ||
    status == ERR_KEY_TOO_LARGE ||
    status == ERR_VALUE_TOO_LARGE ||
    status == ERR_EMPTY_TOPIC ||
    status == ERR_TOPIC_TOO_LARGE ||
    status == ERR_DATA_TOO_LARGE ||
    status == ERR_INPUT_TOO_LARGE ||
    status == ERR_CAPABILITY_LIMIT ||
    status == ERR_CAPABILITY_BYTES ||
    status == ERR_BUFFER_TOO_SMALL ||
    status == ERR_OVERFLOW ||
    status == ERR_UNDERFLOW
  ) {
    return ERR_BOUNDS;
  }
  return ERR_INVALID;
}

/** Names one status for a diagnostic that never reaches consensus. */
export function statusName(status: i32): string {
  if (status == OK) return "LXP_PROGRAM_OK";
  if (status == ERR_DENIED) return "LXP_PROGRAM_ERR_DENIED";
  if (status == ERR_INVALID) return "LXP_PROGRAM_ERR_INVALID";
  if (status == ERR_BOUNDS) return "LXP_PROGRAM_ERR_BOUNDS";
  if (status == ERR_METER) return "LXP_PROGRAM_ERR_METER";
  if (status == ERR_EVIDENCE) return "LXP_PROGRAM_ERR_EVIDENCE";
  if (status == ERR_NULL_ARGUMENT) return "LXP_PROGRAM_ERR_NULL_ARGUMENT";
  if (status == ERR_EMPTY_KEY) return "LXP_PROGRAM_ERR_EMPTY_KEY";
  if (status == ERR_KEY_TOO_LARGE) return "LXP_PROGRAM_ERR_KEY_TOO_LARGE";
  if (status == ERR_VALUE_TOO_LARGE) return "LXP_PROGRAM_ERR_VALUE_TOO_LARGE";
  if (status == ERR_EMPTY_TOPIC) return "LXP_PROGRAM_ERR_EMPTY_TOPIC";
  if (status == ERR_TOPIC_TOO_LARGE) return "LXP_PROGRAM_ERR_TOPIC_TOO_LARGE";
  if (status == ERR_DATA_TOO_LARGE) return "LXP_PROGRAM_ERR_DATA_TOO_LARGE";
  if (status == ERR_INPUT_TOO_LARGE) return "LXP_PROGRAM_ERR_INPUT_TOO_LARGE";
  if (status == ERR_ZERO_AMOUNT) return "LXP_PROGRAM_ERR_ZERO_AMOUNT";
  if (status == ERR_RESERVED_IDENTIFIER) return "LXP_PROGRAM_ERR_RESERVED_IDENTIFIER";
  if (status == ERR_DUPLICATE_CAPABILITY) return "LXP_PROGRAM_ERR_DUPLICATE_CAPABILITY";
  if (status == ERR_CAPABILITY_LIMIT) return "LXP_PROGRAM_ERR_CAPABILITY_LIMIT";
  if (status == ERR_CAPABILITY_BYTES) return "LXP_PROGRAM_ERR_CAPABILITY_BYTES";
  if (status == ERR_BUFFER_TOO_SMALL) return "LXP_PROGRAM_ERR_BUFFER_TOO_SMALL";
  if (status == ERR_RECEIPT_ENCODING) return "LXP_PROGRAM_ERR_RECEIPT_ENCODING";
  if (status == ERR_OVERFLOW) return "LXP_PROGRAM_ERR_OVERFLOW";
  if (status == ERR_UNDERFLOW) return "LXP_PROGRAM_ERR_UNDERFLOW";
  return "LXP_PROGRAM_ERR_UNKNOWN";
}
