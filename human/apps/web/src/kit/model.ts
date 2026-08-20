import { copyEntry } from "../../copy/catalog.ts";
import { formatCopy } from "../../copy/format.ts";

export const PATTERN_PAIRS = [
  "navigation",
  "primary-action",
  "confirmation",
  "detail",
  "filters",
  "money-lists",
  "wizards",
  "search",
  "code-entry",
  "notifications",
] as const;

export const kit = Object.freeze({ patterns: PATTERN_PAIRS });

export const STATUS_PRESENTATIONS = Object.freeze({
  getting_ready: { copyKey: "status.getting_ready", tone: "neutral" },
  sending: { copyKey: "status.sending", tone: "accent" },
  processing: { copyKey: "status.processing", tone: "accent" },
  done: { copyKey: "status.done", tone: "success" },
  done_finalised: { copyKey: "status.done_finalised", tone: "success" },
  still_checking: { copyKey: "status.still_checking", tone: "warning" },
  refused: { copyKey: "status.refused", tone: "destructive" },
  waiting_for_you: { copyKey: "status.waiting_for_you", tone: "warning" },
  waiting_for_wallet: { copyKey: "status.waiting_for_wallet", tone: "neutral" },
  confirming_on_paxeer: { copyKey: "status.confirming_on_paxeer", tone: "accent" },
  crediting: { copyKey: "status.crediting", tone: "accent" },
  waiting_for_settlement: { copyKey: "status.waiting_for_settlement", tone: "warning" },
  ready_to_claim: { copyKey: "status.ready_to_claim", tone: "accent" },
  paid_out: { copyKey: "status.paid_out", tone: "success" },
} as const);

export type StatusKey = keyof typeof STATUS_PRESENTATIONS;
export type StatusTone = (typeof STATUS_PRESENTATIONS)[StatusKey]["tone"];
export type MoneyDirection = "inbound" | "outbound" | "other";
export type ConfirmationKind = "reversible" | "destructive" | "irreversible";
declare const protocolAmountBrand: unique symbol;
export type ProtocolAmount = number & Readonly<{ [protocolAmountBrand]: "ProtocolAmount" }>;

export function protocolAmount(value: number): ProtocolAmount {
  if (!Number.isSafeInteger(value)) {
    throw new RangeError("Protocol amounts must be safe integers");
  }
  return value as ProtocolAmount;
}

export function statusPresentation(status: StatusKey): Readonly<{ label: string; tone: StatusTone }> {
  const presentation = STATUS_PRESENTATIONS[status];
  return Object.freeze({
    label: copyEntry(presentation.copyKey).message,
    tone: presentation.tone,
  });
}

export function directionWord(direction: MoneyDirection): string {
  return formatCopy("movement.direction", { direction });
}

export function confirmationVariant(kind: ConfirmationKind): "primary" | "destructive" {
  return kind === "reversible" ? "primary" : "destructive";
}

export function typedConfirmationReady(
  kind: ConfirmationKind,
  typed?: Readonly<{ expectedValue: string; value: string }>,
): boolean {
  return kind !== "irreversible" ||
    (typed !== undefined && typed.expectedValue.length > 0 && typed.value === typed.expectedValue);
}

export function feeMathTotal(steps: readonly Readonly<{ amount: ProtocolAmount }>[]): ProtocolAmount {
  return steps.reduce(
    (total, step) => protocolAmount(total + step.amount),
    protocolAmount(0),
  );
}
