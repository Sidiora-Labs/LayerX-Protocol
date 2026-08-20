export const COPY_CATALOG_VERSION = "1.0.0" as const;

export const BANNED_VOCABULARY = [
  "DID",
  "session key",
  "capability",
  "nullifier",
  "checkpoint",
  "payload",
  "canonical",
  "idempotency",
  "attestation",
  "proof",
] as const;

export const DATE_FORMAT = "dd MMM yyyy, HH:mm z" as const;
export const AMOUNT_FORMAT = "{sign}{amount} {currencyCode}" as const;

export type CopySurface = "default" | "technical" | "explorer";
export type CopyKind = "action" | "body" | "format" | "status";

export interface CopyEntry {
  readonly key: string;
  readonly message: string;
  readonly context: string;
  readonly surface: CopySurface;
  readonly kind: CopyKind;
  readonly moneyAdjacent: boolean;
}

export const copyEntries = [
  { key: "application.name", message: "LayerX Human Interface", context: "Accessible product name and application metadata.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "status.getting_ready", message: "Getting ready", context: "Prepared activity that has not been signed.", surface: "default", kind: "status", moneyAdjacent: true },
  { key: "status.sending", message: "Sending", context: "Submitted activity without a receipt yet.", surface: "default", kind: "status", moneyAdjacent: true },
  { key: "status.processing", message: "Processing", context: "Executed activity awaiting final settlement.", surface: "default", kind: "status", moneyAdjacent: true },
  { key: "status.done", message: "Done", context: "Shown only when a LayerX receipt verifies.", surface: "default", kind: "status", moneyAdjacent: true },
  { key: "status.done_finalised", message: "Done, finalised", context: "Receipt whose settlement finality verifies.", surface: "default", kind: "status", moneyAdjacent: true },
  { key: "status.still_checking", message: "Still checking — don't send again", context: "Unknown submission outcome; duplicate controls stay locked.", surface: "default", kind: "status", moneyAdjacent: true },
  { key: "status.refused", message: "Didn't go through", context: "Typed refusal or failure, paired with whether money moved.", surface: "default", kind: "status", moneyAdjacent: true },
  { key: "status.waiting_for_you", message: "Waiting for you", context: "Activity held for a human decision.", surface: "default", kind: "status", moneyAdjacent: true },
  { key: "status.waiting_for_wallet", message: "Waiting for wallet", context: "Deposit is waiting for the explicit wallet signature.", surface: "default", kind: "status", moneyAdjacent: true },
  { key: "status.confirming_on_paxeer", message: "Confirming on Paxeer", context: "Custody transaction is progressing toward Paxeer finality.", surface: "default", kind: "status", moneyAdjacent: true },
  { key: "status.crediting", message: "Crediting", context: "Final custody evidence is being credited on LayerX.", surface: "default", kind: "status", moneyAdjacent: true },
  { key: "status.waiting_for_settlement", message: "Waiting for settlement", context: "Withdrawal debit is waiting for its settlement window.", surface: "default", kind: "status", moneyAdjacent: true },
  { key: "status.ready_to_claim", message: "Ready to claim", context: "Withdrawal can now be claimed in the bound wallet.", surface: "default", kind: "status", moneyAdjacent: true },
  { key: "status.paid_out", message: "Paid out", context: "Paxeer payout transaction reached verified finality.", surface: "default", kind: "status", moneyAdjacent: true },
  { key: "approval.count", message: "{count, plural, =0 {No approvals waiting} one {# approval waiting} other {# approvals waiting}}", context: "Approval badge and inbox count with ICU plural handling.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "approval.activity.move_money", message: "Move {amount} {asset} to {counterparty}. Fees can be up to {fee}.", context: "Digest-bound asset movement approval content.", surface: "default", kind: "body", moneyAdjacent: true },
  { key: "approval.activity.add_money", message: "Add {amount} {asset} to {counterparty}. Fees can be up to {fee}.", context: "Digest-bound Paxeer credit approval content.", surface: "default", kind: "body", moneyAdjacent: true },
  { key: "approval.activity.withdrawal_wallet", message: "Use {counterparty} for withdrawals. Fees can be up to {fee}.", context: "Digest-bound payout wallet approval content.", surface: "default", kind: "body", moneyAdjacent: true },
  { key: "approval.activity.unrenderable", message: "This request cannot be reviewed here.", context: "Safe non-approvable copy for an unknown activity class.", surface: "default", kind: "body", moneyAdjacent: true },
  { key: "movement.direction", message: "{direction, select, inbound {Money in} outbound {Money out} other {Money moved}}", context: "Accessible direction wording selected independently of color.", surface: "default", kind: "body", moneyAdjacent: true },
  { key: "state.loading", message: "Getting ready", context: "Screen data is loading from the real service.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "state.empty", message: "Nothing here yet", context: "Screen has loaded successfully without records.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "state.error", message: "Something went wrong", context: "Screen could not load and offers recovery actions.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "state.offline", message: "You're offline", context: "Screen cannot reach the service and offers retry.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "state.degraded", message: "Some information may be delayed", context: "Screen is usable with explicitly stale information.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "action.retry", message: "Retry", context: "Retry the current read without duplicating a money movement.", surface: "default", kind: "action", moneyAdjacent: false },
  { key: "action.reload", message: "Reload", context: "Reload a structurally broken screen.", surface: "default", kind: "action", moneyAdjacent: false },
  { key: "action.report", message: "Report", context: "Submit consented non-sensitive diagnostic context.", surface: "default", kind: "action", moneyAdjacent: false },
] as const satisfies readonly CopyEntry[];

const catalog = new Map<string, CopyEntry>(
  copyEntries.map((entry): [string, CopyEntry] => [entry.key, entry]),
);

export function human_copy_catalog(): ReadonlyMap<string, CopyEntry> {
  return catalog;
}

export function copyEntry(key: string): CopyEntry {
  const entry = catalog.get(key);
  if (entry === undefined) {
    throw new Error(`Unknown copy key: ${key}`);
  }
  return entry;
}
