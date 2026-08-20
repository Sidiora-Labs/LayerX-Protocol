export const COPY_CATALOG_VERSION = "1.1.0" as const;

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
  { key: "navigation.home", message: "Home", context: "Authenticated application home navigation and heading.", surface: "default", kind: "action", moneyAdjacent: false },
  { key: "navigation.agents", message: "Agents", context: "Managed agents navigation.", surface: "default", kind: "action", moneyAdjacent: false },
  { key: "navigation.activity", message: "Activity", context: "Account activity navigation.", surface: "default", kind: "action", moneyAdjacent: false },
  { key: "navigation.more", message: "More", context: "Additional application destinations navigation.", surface: "default", kind: "action", moneyAdjacent: false },
  { key: "app.home.summary", message: "Your account tools and recent activity are available here.", context: "Authenticated plane home introduction.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "explorer.title", message: "LayerX Explorer", context: "Public explorer page heading.", surface: "explorer", kind: "body", moneyAdjacent: false },
  { key: "explorer.summary", message: "Look up public LayerX activity and verify its records.", context: "Public explorer introduction.", surface: "explorer", kind: "body", moneyAdjacent: false },
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
  { key: "state.loading.body", message: "Your information is being checked.", context: "Layout-reserving loading state explanation.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "state.empty", message: "Nothing here yet", context: "Screen has loaded successfully without records.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "state.empty.body", message: "New items will appear here when they are available.", context: "Loaded empty state explanation.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "state.error", message: "Something went wrong", context: "Screen could not load and offers recovery actions.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "state.error.body", message: "This screen could not be shown.", context: "Error state explanation without exposing internal details.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "state.offline", message: "You're offline", context: "Screen cannot reach the service and offers retry.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "state.offline.body", message: "Reconnect to refresh this screen.", context: "Per-screen offline recovery guidance.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "state.offline.banner", message: "You're offline. Information may be out of date.", context: "Persistent application-wide offline banner.", surface: "default", kind: "status", moneyAdjacent: false },
  { key: "state.degraded", message: "Some information may be delayed", context: "Screen is usable with explicitly stale information.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "state.degraded.age", message: "Last updated {age}.", context: "Age of stale information on a degraded screen.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "state.degraded.reference", message: "Current view is based on {referencePoint}.", context: "Reference point for stale information on a degraded screen.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "state.still_checking.body", message: "Your money is safe while the result is checked. Do not send again.", context: "Unknown money outcome guidance while automatic lookup continues.", surface: "default", kind: "body", moneyAdjacent: true },
  { key: "state.still_checking.locked", message: "Sending again is locked until the result is known.", context: "Disabled reason for duplicate money controls.", surface: "default", kind: "body", moneyAdjacent: true },
  { key: "error.money.not_started", message: "This screen error did not start a new money movement.", context: "Money consequence for a route rendering failure.", surface: "default", kind: "body", moneyAdjacent: true },
  { key: "error.technical.title", message: "Technical details", context: "Collapsed diagnostic disclosure title.", surface: "technical", kind: "body", moneyAdjacent: false },
  { key: "error.technical.machine_code", message: "Code: {machineCode}", context: "Non-sensitive machine-readable error code.", surface: "technical", kind: "body", moneyAdjacent: false },
  { key: "error.technical.trace", message: "Trace: {traceId}", context: "Non-sensitive trace identifier for support.", surface: "technical", kind: "body", moneyAdjacent: false },
  { key: "report.consent.title", message: "Send an error report?", context: "Consent confirmation title before submitting diagnostic context.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "report.consent.body", message: "This sends the trace, error code, and screen name. It does not include names, balances, or entered values.", context: "Explicit scope of consented redacted diagnostic submission.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "report.submit", message: "Send report", context: "Consent action for a redacted diagnostic report.", surface: "default", kind: "action", moneyAdjacent: false },
  { key: "report.saved", message: "Report saved", context: "Confirmation after the server durably accepts a diagnostic report.", surface: "default", kind: "status", moneyAdjacent: false },
  { key: "report.saved.body", message: "Support can use the trace shown in technical details.", context: "Server-confirmed diagnostic report detail.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "report.pending", message: "Report waiting for connection", context: "Explicit pending status for an offline diagnostic report.", surface: "default", kind: "status", moneyAdjacent: false },
  { key: "report.pending.body", message: "It will be sent when you're back online.", context: "Offline report queue and reconnect behavior.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "report.failed", message: "The report could not be saved. You can try again.", context: "Honest diagnostic report submission failure.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "queue.waiting", message: "This update will continue when you're online.", context: "Honest queued non-money action status.", surface: "default", kind: "status", moneyAdjacent: false },
  { key: "queue.money.not_queued", message: "Money movements are never queued. Return when you're online.", context: "Offline money safety explanation.", surface: "default", kind: "body", moneyAdjacent: true },
  { key: "action.retry", message: "Retry", context: "Retry the current read without duplicating a money movement.", surface: "default", kind: "action", moneyAdjacent: false },
  { key: "action.reload", message: "Reload", context: "Reload a structurally broken screen.", surface: "default", kind: "action", moneyAdjacent: false },
  { key: "action.report", message: "Report", context: "Submit consented non-sensitive diagnostic context.", surface: "default", kind: "action", moneyAdjacent: false },
  { key: "action.send_again", message: "Send again", context: "Duplicate money action shown disabled while an outcome is unknown.", surface: "default", kind: "action", moneyAdjacent: true },
  { key: "action.dismiss", message: "Dismiss", context: "Dismiss a completed status message.", surface: "default", kind: "action", moneyAdjacent: false },
  { key: "action.cancel", message: "Cancel", context: "Dismiss a confirmation without applying its action.", surface: "default", kind: "action", moneyAdjacent: false },
  { key: "action.copy", message: "Copy", context: "Copy an explicitly copyable identifier.", surface: "default", kind: "action", moneyAdjacent: false },
  { key: "action.copied", message: "Copied", context: "Confirm that an identifier was copied.", surface: "default", kind: "action", moneyAdjacent: false },
  { key: "action.copy_failed", message: "Couldn’t copy", context: "Announce that an identifier could not be copied.", surface: "default", kind: "status", moneyAdjacent: false },
  { key: "confirmation.incomplete", message: "Confirmation is incomplete.", context: "Explain why an irreversible confirmation remains disabled.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "confirmation.type_exact", message: "Type {expectedValue} exactly to continue.", context: "Disabled reason for an irreversible typed confirmation.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "confirmation.type_to_confirm", message: "Type {expectedValue} to confirm", context: "Label for an irreversible typed-confirmation field.", surface: "default", kind: "body", moneyAdjacent: false },
  { key: "fee.disclosure.title", message: "How this fee is calculated", context: "Heading for the numbered fee calculation disclosure.", surface: "default", kind: "body", moneyAdjacent: true },
  { key: "fee.disclosure.total", message: "Total fee", context: "Label for the computed total in a fee calculation disclosure.", surface: "default", kind: "body", moneyAdjacent: true },
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
