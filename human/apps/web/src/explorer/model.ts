export type ExplorerVerificationLevel =
  | "unverified"
  | "sequencer-signed"
  | "batch-included"
  | "state-proven"
  | "checkpoint-finalised"
  | "settlement-anchored";

export interface ExplorerFreshness {
  readonly observedChainSequence: string;
  readonly observedSealedBatch: string;
  readonly observedFinalisedCheckpoint: string;
  readonly indexedBatch?: string;
  readonly indexedCheckpoint?: string;
  readonly batchesBehind: string;
  readonly current: boolean;
}

export interface CheckpointRecord {
  readonly checkpointId: string;
  readonly batchNumber: string;
  readonly firstSequence: string;
  readonly lastSequence: string;
  readonly achievedSignatures: string;
  readonly requiredSignatures: string;
  readonly verificationLevel: ExplorerVerificationLevel;
}

export interface BatchRecord {
  readonly batchNumber: string;
  readonly totalAvailabilityBytes: string;
  readonly activityCount: string;
  readonly receiptCount: string;
  readonly eventCount: string;
  readonly checkpointId?: string;
  readonly verificationLevel: ExplorerVerificationLevel;
}

export interface ReceiptRecord {
  readonly receiptId: string;
  readonly batchNumber: string;
  readonly ordinal: string;
  readonly canonicalBytes: string;
  readonly verificationLevel: ExplorerVerificationLevel;
}

export interface AccountActivityRecord {
  readonly receiptId: string;
  readonly receiptDigest: string;
  readonly batchNumber: string;
  readonly globalSequence: string;
  readonly activityId: string;
  readonly operation: string;
  readonly resultCode: string;
  readonly asset: string;
  readonly amount: string;
  readonly from: string;
  readonly to: string;
  readonly verificationLevel: ExplorerVerificationLevel;
}

export interface ExplorerPage<T> {
  readonly items: readonly T[];
  readonly nextBefore?: string;
  readonly freshness: ExplorerFreshness;
}

export interface ExplorerRecord<T> {
  readonly value?: T;
  readonly freshness: ExplorerFreshness;
}

export interface EvidenceVerificationReport {
  readonly kind: "receipt" | "activity-inclusion" | "state-inclusion";
  readonly achievedLevel: ExplorerVerificationLevel;
  readonly receiptDigest?: string;
  readonly headerDigest?: string;
  readonly proofRoot?: string;
  readonly freshness: ExplorerFreshness;
}

type JsonRecord = Readonly<Record<string, unknown>>;

function record(value: unknown, at: string): JsonRecord {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError(`${at} must be an object`);
  }
  return value as JsonRecord;
}

function text(value: unknown, at: string): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new TypeError(`${at} must be a non-empty string`);
  }
  return value;
}

function decimal(value: unknown, at: string): string {
  const candidate = text(value, at);
  if (!/^(?:0|[1-9]\d*)$/u.test(candidate)) {
    throw new TypeError(`${at} must be an unsigned decimal string`);
  }
  return candidate;
}

function signedDecimal(value: unknown, at: string): string {
  const candidate = text(value, at);
  if (!/^(?:0|-?[1-9]\d*)$/u.test(candidate)) {
    throw new TypeError(`${at} must be a signed decimal string`);
  }
  return candidate;
}

function hex(value: unknown, at: string): string {
  const candidate = text(value, at).toLowerCase();
  if (!/^[0-9a-f]{64}$/u.test(candidate)) {
    throw new TypeError(`${at} must be a 32-byte lowercase hex identifier`);
  }
  return candidate;
}

function optionalHex(value: unknown, at: string): string | undefined {
  return value === undefined || value === null ? undefined : hex(value, at);
}

function boolean(value: unknown, at: string): boolean {
  if (typeof value !== "boolean") {
    throw new TypeError(`${at} must be a boolean`);
  }
  return value;
}

function verificationLevel(value: unknown, at: string): ExplorerVerificationLevel {
  const candidate = text(value, at);
  if (
    candidate !== "unverified"
    && candidate !== "sequencer-signed"
    && candidate !== "batch-included"
    && candidate !== "state-proven"
    && candidate !== "checkpoint-finalised"
    && candidate !== "settlement-anchored"
  ) {
    throw new TypeError(`${at} is not a declared verification level`);
  }
  return candidate;
}

export function decodeFreshness(value: unknown, at = "freshness"): ExplorerFreshness {
  const item = record(value, at);
  const indexedCheckpoint = optionalHex(item.indexed_checkpoint, `${at}.indexed_checkpoint`);
  return Object.freeze({
    observedChainSequence: decimal(item.observed_chain_sequence, `${at}.observed_chain_sequence`),
    observedSealedBatch: decimal(item.observed_sealed_batch, `${at}.observed_sealed_batch`),
    observedFinalisedCheckpoint: hex(
      item.observed_finalised_checkpoint,
      `${at}.observed_finalised_checkpoint`,
    ),
    ...(indexedCheckpoint === undefined ? {} : { indexedCheckpoint }),
    ...(item.indexed_batch === undefined || item.indexed_batch === null
      ? {}
      : { indexedBatch: decimal(item.indexed_batch, `${at}.indexed_batch`) }),
    batchesBehind: decimal(item.batches_behind, `${at}.batches_behind`),
    current: boolean(item.current, `${at}.current`),
  });
}

export function decodeCheckpoint(value: unknown, at = "checkpoint"): CheckpointRecord {
  const item = record(value, at);
  return Object.freeze({
    checkpointId: hex(item.checkpoint_id, `${at}.checkpoint_id`),
    batchNumber: decimal(item.batch_number, `${at}.batch_number`),
    firstSequence: decimal(item.first_sequence, `${at}.first_sequence`),
    lastSequence: decimal(item.last_sequence, `${at}.last_sequence`),
    achievedSignatures: decimal(item.achieved_signatures, `${at}.achieved_signatures`),
    requiredSignatures: decimal(item.required_signatures, `${at}.required_signatures`),
    verificationLevel: verificationLevel(item.verification_level, `${at}.verification_level`),
  });
}

export function decodeBatch(value: unknown, at = "batch"): BatchRecord {
  const item = record(value, at);
  const checkpointId = optionalHex(item.checkpoint_id, `${at}.checkpoint_id`);
  return Object.freeze({
    batchNumber: decimal(item.batch_number, `${at}.batch_number`),
    totalAvailabilityBytes: decimal(
      item.total_availability_bytes,
      `${at}.total_availability_bytes`,
    ),
    activityCount: decimal(item.activity_count, `${at}.activity_count`),
    receiptCount: decimal(item.receipt_count, `${at}.receipt_count`),
    eventCount: decimal(item.event_count, `${at}.event_count`),
    ...(checkpointId === undefined ? {} : { checkpointId }),
    verificationLevel: verificationLevel(item.verification_level, `${at}.verification_level`),
  });
}

export function decodeReceipt(value: unknown, at = "receipt"): ReceiptRecord {
  const item = record(value, at);
  const canonicalBytes = text(item.canonical_bytes, `${at}.canonical_bytes`);
  if (!/^[A-Za-z0-9_-]+={0,2}$/u.test(canonicalBytes) || canonicalBytes.length > 1_500_000) {
    throw new TypeError(`${at}.canonical_bytes must be bounded base64url`);
  }
  return Object.freeze({
    receiptId: hex(item.receipt_id, `${at}.receipt_id`),
    batchNumber: decimal(item.batch_number, `${at}.batch_number`),
    ordinal: decimal(item.ordinal, `${at}.ordinal`),
    canonicalBytes,
    verificationLevel: verificationLevel(item.verification_level, `${at}.verification_level`),
  });
}

export function decodeAccountActivity(
  value: unknown,
  at = "account_activity",
): AccountActivityRecord {
  const item = record(value, at);
  return Object.freeze({
    receiptId: hex(item.receipt_id, `${at}.receipt_id`),
    receiptDigest: hex(item.receipt_digest, `${at}.receipt_digest`),
    batchNumber: decimal(item.batch_number, `${at}.batch_number`),
    globalSequence: decimal(item.global_sequence, `${at}.global_sequence`),
    activityId: hex(item.activity_id, `${at}.activity_id`),
    operation: decimal(item.operation, `${at}.operation`),
    resultCode: signedDecimal(item.result_code, `${at}.result_code`),
    asset: hex(item.asset, `${at}.asset`),
    amount: decimal(item.amount, `${at}.amount`),
    from: hex(item.from, `${at}.from`),
    to: hex(item.to, `${at}.to`),
    verificationLevel: verificationLevel(item.verification_level, `${at}.verification_level`),
  });
}

export function decodePage<T>(
  value: unknown,
  decodeItem: (value: unknown, at: string) => T,
  at = "page",
): ExplorerPage<T> {
  const item = record(value, at);
  if (!Array.isArray(item.items)) {
    throw new TypeError(`${at}.items must be an array`);
  }
  return Object.freeze({
    items: Object.freeze(item.items.map((entry, index) => decodeItem(entry, `${at}.items[${String(index)}]`))),
    ...(item.next_before === undefined || item.next_before === null
      ? {}
      : { nextBefore: decimal(item.next_before, `${at}.next_before`) }),
    freshness: decodeFreshness(item.freshness, `${at}.freshness`),
  });
}

export function decodeRecord<T>(
  value: unknown,
  decodeValue: (value: unknown, at: string) => T,
  at = "record",
): ExplorerRecord<T> {
  const item = record(value, at);
  return Object.freeze({
    ...(item.value === undefined || item.value === null
      ? {}
      : { value: decodeValue(item.value, `${at}.value`) }),
    freshness: decodeFreshness(item.freshness, `${at}.freshness`),
  });
}

export function decodeVerificationReport(
  value: unknown,
  at = "verification",
): EvidenceVerificationReport {
  const item = record(value, at);
  const kind = text(item.kind, `${at}.kind`);
  if (kind !== "receipt" && kind !== "activity-inclusion" && kind !== "state-inclusion") {
    throw new TypeError(`${at}.kind is not supported`);
  }
  const receiptDigest = optionalHex(item.receipt_digest, `${at}.receipt_digest`);
  const headerDigest = optionalHex(item.header_digest, `${at}.header_digest`);
  const proofRoot = optionalHex(item.proof_root, `${at}.proof_root`);
  return Object.freeze({
    kind,
    achievedLevel: verificationLevel(item.achieved_level, `${at}.achieved_level`),
    ...(receiptDigest === undefined ? {} : { receiptDigest }),
    ...(headerDigest === undefined ? {} : { headerDigest }),
    ...(proofRoot === undefined ? {} : { proofRoot }),
    freshness: decodeFreshness(item.freshness, `${at}.freshness`),
  });
}

export function encodeFreshness(freshness: ExplorerFreshness): Readonly<Record<string, unknown>> {
  return Object.freeze({
    observed_chain_sequence: freshness.observedChainSequence,
    observed_sealed_batch: freshness.observedSealedBatch,
    observed_finalised_checkpoint: freshness.observedFinalisedCheckpoint,
    indexed_batch: freshness.indexedBatch ?? null,
    indexed_checkpoint: freshness.indexedCheckpoint ?? null,
    batches_behind: freshness.batchesBehind,
    current: freshness.current,
  });
}

export function encodeVerificationReport(
  report: EvidenceVerificationReport,
): Readonly<Record<string, unknown>> {
  return Object.freeze({
    kind: report.kind,
    achieved_level: report.achievedLevel,
    receipt_digest: report.receiptDigest ?? null,
    header_digest: report.headerDigest ?? null,
    proof_root: report.proofRoot ?? null,
    freshness: encodeFreshness(report.freshness),
  });
}

export function validExplorerIdentifier(value: string): boolean {
  return /^[0-9a-fA-F]{64}$/u.test(value);
}

export function validExplorerCoordinate(value: string): boolean {
  return /^(?:0|[1-9]\d*)$/u.test(value);
}

export function explorer(): Readonly<{ publicOnly: true; maximumPageSize: 100 }> {
  return Object.freeze({ publicOnly: true, maximumPageSize: 100 });
}

export function human_web_explorer() {
  return explorer();
}
