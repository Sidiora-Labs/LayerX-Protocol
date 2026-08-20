import { PlatformSdkError } from "./production.js";

const MERKLE_LEAF_DOMAIN = new TextEncoder().encode("LXP/v1/merkle-leaf\0");
const MERKLE_INTERNAL_DOMAIN = new TextEncoder().encode("LXP/v1/merkle-internal\0");
const BATCH_HEADER_DOMAIN = new TextEncoder().encode("LXP/v1/batch-header\0");
const CHECKPOINT_DOMAIN = new TextEncoder().encode("LXP/v1/checkpoint-certificate\0");
const BATCH_HEADER_BYTES = 354;
const ALL_AVAILABILITY_CLASSES = 0x1f;

export interface MerkleProof {
  readonly leafIndex: number;
  readonly leafCount: number;
  readonly siblings: readonly Uint8Array[];
}

export interface BatchHeader {
  readonly protocolVersion: number;
  readonly networkId: number;
  readonly epoch: bigint;
  readonly batchNumber: bigint;
  readonly firstSequence: bigint;
  readonly lastSequence: bigint;
  readonly previousStateRoot: Uint8Array;
  readonly resultingStateRoot: Uint8Array;
  readonly activityMerkleRoot: Uint8Array;
  readonly receiptMerkleRoot: Uint8Array;
  readonly eventMerkleRoot: Uint8Array;
  readonly dataAvailabilityRoot: Uint8Array;
  readonly oracleRoot: Uint8Array;
  readonly timestampMs: bigint;
  readonly sequencerId: Uint8Array;
}

export interface SequencerAuthorization {
  readonly sequencerId: Uint8Array;
  readonly publicKey: Uint8Array;
  readonly firstBatchNumber: bigint;
  readonly lastBatchNumber: bigint;
}

export type InclusionKind = "activity" | "receipt" | "event" | "state";

export interface InclusionVerification {
  readonly level: "batch-included" | "state-proven";
  readonly header: BatchHeader;
  readonly headerDigest: Uint8Array;
  readonly root: Uint8Array;
}

export interface CheckpointAttestation {
  readonly checkpointId: Uint8Array;
  readonly checkpointHash: Uint8Array;
  readonly guarantorId: Uint8Array;
  readonly batchNumber: bigint;
  readonly dataAvailabilityRoot: Uint8Array;
  readonly replayed: boolean;
  readonly dataPossessed: boolean;
  readonly availabilityClassMask: number;
  readonly attestedAtMs: bigint;
  readonly signature: Uint8Array;
}

export interface GuarantorKey {
  readonly guarantorId: Uint8Array;
  readonly publicKey: Uint8Array;
  readonly bonded: boolean;
}

export interface CheckpointCertificate {
  readonly canonicalHeader: Uint8Array;
  readonly validityProof: Uint8Array;
  readonly attestations: readonly CheckpointAttestation[];
  readonly threshold: number;
  readonly settlementReference?: Uint8Array;
}

export interface CheckpointVerificationInput {
  readonly certificate: CheckpointCertificate;
  readonly bondedSet: readonly GuarantorKey[];
  readonly registeredCheckpointId: Uint8Array;
  readonly registeredSettlementReference?: Uint8Array;
  readonly availabilityObtained: boolean;
}

export interface CheckpointVerification {
  readonly level: "checkpoint-finalised" | "settlement-anchored";
  readonly checkpointId: Uint8Array;
  readonly achieved: number;
  readonly required: number;
  readonly header: BatchHeader;
}

export interface LocalSignatureVerifier {
  verifySecp256k1(publicKey: Uint8Array, signature: Uint8Array, digest: Uint8Array): Promise<boolean>;
}

function verificationFailure(): never {
  throw new PlatformSdkError({ code: "verification-failure", retry: "never" });
}

function exactBytes(value: Uint8Array, length: number): Uint8Array {
  if (value.length !== length) {
    return verificationFailure();
  }
  return value;
}

function arrayBuffer(value: Uint8Array): ArrayBuffer {
  const copy = new ArrayBuffer(value.length);
  new Uint8Array(copy).set(value);
  return copy;
}

function concatenate(...values: readonly Uint8Array[]): Uint8Array {
  const length = values.reduce((total, value) => total + value.length, 0);
  const result = new Uint8Array(length);
  let offset = 0;
  for (const value of values) {
    result.set(value, offset);
    offset += value.length;
  }
  return result;
}

async function sha256(...values: readonly Uint8Array[]): Promise<Uint8Array> {
  const digest = await globalThis.crypto.subtle.digest("SHA-256", arrayBuffer(concatenate(...values)));
  return new Uint8Array(digest);
}

function equal(left: Uint8Array, right: Uint8Array): boolean {
  if (left.length !== right.length) {
    return false;
  }
  let difference = 0;
  for (let index = 0; index < left.length; index += 1) {
    difference |= (left[index] ?? 0) ^ (right[index] ?? 0);
  }
  return difference === 0;
}

function proofDepth(leafCount: number): number {
  let count = leafCount;
  let depth = 0;
  while (count > 1) {
    count = Math.ceil(count / 2);
    depth += 1;
  }
  return depth;
}

async function leafHash(bytes: Uint8Array): Promise<Uint8Array> {
  return sha256(MERKLE_LEAF_DOMAIN, bytes);
}

async function nodeHash(left: Uint8Array, right: Uint8Array): Promise<Uint8Array> {
  return sha256(MERKLE_INTERNAL_DOMAIN, exactBytes(left, 32), exactBytes(right, 32));
}

export async function verifyMerkleInclusion(
  canonicalLeaf: Uint8Array,
  proof: MerkleProof,
  expectedRoot: Uint8Array,
): Promise<void> {
  if (
    !Number.isSafeInteger(proof.leafIndex)
    || !Number.isSafeInteger(proof.leafCount)
    || proof.leafCount <= 0
    || proof.leafCount > 0xffff_ffff
    || proof.leafIndex < 0
    || proof.leafIndex > 0xffff_ffff
    || proof.leafIndex >= proof.leafCount
    || proof.siblings.length > 32
    || proof.siblings.length !== proofDepth(proof.leafCount)
  ) {
    return verificationFailure();
  }
  let current = await leafHash(canonicalLeaf);
  let index = proof.leafIndex;
  let levelCount = proof.leafCount;
  for (const siblingValue of proof.siblings) {
    const sibling = exactBytes(siblingValue, 32);
    if ((index ^ 1) >= levelCount && !equal(sibling, current)) {
      return verificationFailure();
    }
    current = index % 2 === 0
      ? await nodeHash(current, sibling)
      : await nodeHash(sibling, current);
    index = Math.floor(index / 2);
    levelCount = Math.ceil(levelCount / 2);
  }
  if (!equal(current, exactBytes(expectedRoot, 32))) {
    return verificationFailure();
  }
}

class Decoder {
  #offset = 0;

  public constructor(private readonly bytes: Uint8Array) {}

  public finish(): void {
    if (this.#offset !== this.bytes.length) {
      return verificationFailure();
    }
  }

  public u8(): number {
    const value = this.bytes[this.#offset];
    if (value === undefined) {
      return verificationFailure();
    }
    this.#offset += 1;
    return value;
  }

  public u16(): number {
    return Number(this.integer(2));
  }

  public u32(): number {
    return Number(this.integer(4));
  }

  public u64(): bigint {
    return this.integer(8);
  }

  public fixed(length: number): Uint8Array {
    const end = this.#offset + length;
    if (!Number.isSafeInteger(end) || end > this.bytes.length) {
      return verificationFailure();
    }
    const value = this.bytes.slice(this.#offset, end);
    this.#offset = end;
    return value;
  }

  public bounded(length: number): Uint8Array {
    if (this.u32() !== length) {
      return verificationFailure();
    }
    return this.fixed(length);
  }

  private integer(length: number): bigint {
    const bytes = this.fixed(length);
    let value = 0n;
    for (const byte of bytes) {
      value = (value << 8n) | BigInt(byte);
    }
    return value;
  }
}

function field(decoder: Decoder, expected: number): void {
  if (decoder.u8() !== expected) {
    return verificationFailure();
  }
}

export function decodeBatchHeader(canonicalHeader: Uint8Array): BatchHeader {
  if (canonicalHeader.length !== BATCH_HEADER_BYTES) {
    return verificationFailure();
  }
  const decoder = new Decoder(canonicalHeader);
  if (decoder.u16() !== 1 || decoder.u16() !== 0x1701 || decoder.u8() !== 15) {
    return verificationFailure();
  }
  field(decoder, 1);
  const protocolVersion = decoder.u16();
  field(decoder, 2);
  const networkId = decoder.u32();
  field(decoder, 3);
  const epoch = decoder.u64();
  field(decoder, 4);
  const batchNumber = decoder.u64();
  field(decoder, 5);
  const firstSequence = decoder.u64();
  field(decoder, 6);
  const lastSequence = decoder.u64();
  field(decoder, 7);
  const previousStateRoot = decoder.bounded(32);
  field(decoder, 8);
  const resultingStateRoot = decoder.bounded(32);
  field(decoder, 9);
  const activityMerkleRoot = decoder.bounded(32);
  field(decoder, 10);
  const receiptMerkleRoot = decoder.bounded(32);
  field(decoder, 11);
  const eventMerkleRoot = decoder.bounded(32);
  field(decoder, 12);
  const dataAvailabilityRoot = decoder.bounded(32);
  field(decoder, 13);
  const oracleRoot = decoder.bounded(32);
  field(decoder, 14);
  const timestampMs = decoder.u64();
  field(decoder, 15);
  const sequencerId = decoder.bounded(32);
  decoder.finish();
  return Object.freeze({
    protocolVersion,
    networkId,
    epoch,
    batchNumber,
    firstSequence,
    lastSequence,
    previousStateRoot,
    resultingStateRoot,
    activityMerkleRoot,
    receiptMerkleRoot,
    eventMerkleRoot,
    dataAvailabilityRoot,
    oracleRoot,
    timestampMs,
    sequencerId,
  });
}

async function verifyEd25519(publicKey: Uint8Array, signature: Uint8Array, digest: Uint8Array): Promise<boolean> {
  try {
    const key = await globalThis.crypto.subtle.importKey(
      "raw",
      arrayBuffer(exactBytes(publicKey, 32)),
      { name: "Ed25519" },
      false,
      ["verify"],
    );
    return await globalThis.crypto.subtle.verify(
      { name: "Ed25519" },
      key,
      arrayBuffer(exactBytes(signature, 64)),
      arrayBuffer(exactBytes(digest, 32)),
    );
  } catch {
    return false;
  }
}

function inclusionRoot(header: BatchHeader, kind: InclusionKind): Uint8Array {
  switch (kind) {
    case "activity":
      return header.activityMerkleRoot;
    case "receipt":
      return header.receiptMerkleRoot;
    case "event":
      return header.eventMerkleRoot;
    case "state":
      return header.resultingStateRoot;
  }
}

export async function verifyBatchInclusion(
  kind: InclusionKind,
  canonicalLeaf: Uint8Array,
  proof: MerkleProof,
  canonicalHeader: Uint8Array,
  headerSignature: Uint8Array,
  authorization: SequencerAuthorization,
): Promise<InclusionVerification> {
  const header = decodeBatchHeader(canonicalHeader);
  if (
    header.batchNumber < authorization.firstBatchNumber
    || header.batchNumber > authorization.lastBatchNumber
    || !equal(header.sequencerId, exactBytes(authorization.sequencerId, 32))
  ) {
    return verificationFailure();
  }
  const digest = await sha256(BATCH_HEADER_DOMAIN, canonicalHeader);
  if (!await verifyEd25519(authorization.publicKey, headerSignature, digest)) {
    return verificationFailure();
  }
  const root = inclusionRoot(header, kind);
  await verifyMerkleInclusion(canonicalLeaf, proof, root);
  return Object.freeze({
    level: kind === "state" ? "state-proven" : "batch-included",
    header,
    headerDigest: digest,
    root,
  });
}

function u32(value: number): Uint8Array {
  if (!Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) {
    return verificationFailure();
  }
  const result = new Uint8Array(4);
  new DataView(result.buffer).setUint32(0, value, false);
  return result;
}

function u64(value: bigint): Uint8Array {
  if (value < 0n || value > 0xffff_ffff_ffff_ffffn) {
    return verificationFailure();
  }
  const result = new Uint8Array(8);
  new DataView(result.buffer).setBigUint64(0, value, false);
  return result;
}

function attestationMessage(attestation: CheckpointAttestation): Uint8Array {
  return concatenate(
    exactBytes(attestation.checkpointId, 32),
    exactBytes(attestation.checkpointHash, 32),
    exactBytes(attestation.guarantorId, 32),
    u64(attestation.batchNumber),
    exactBytes(attestation.dataAvailabilityRoot, 32),
    new Uint8Array([
      attestation.replayed ? 1 : 0,
      attestation.dataPossessed ? 1 : 0,
      attestation.availabilityClassMask,
    ]),
    u64(attestation.attestedAtMs),
  );
}

export async function verifyCheckpoint(
  input: CheckpointVerificationInput,
  signatures: LocalSignatureVerifier,
): Promise<CheckpointVerification> {
  const { certificate } = input;
  if (!input.availabilityObtained || certificate.validityProof.length > 0xffff_ffff) {
    return verificationFailure();
  }
  const header = decodeBatchHeader(certificate.canonicalHeader);
  const checkpointId = await sha256(
    CHECKPOINT_DOMAIN,
    certificate.canonicalHeader,
    u32(certificate.validityProof.length),
    certificate.validityProof,
  );
  if (!equal(checkpointId, exactBytes(input.registeredCheckpointId, 32))) {
    return verificationFailure();
  }
  if (certificate.threshold <= 0 || !Number.isSafeInteger(certificate.threshold)) {
    return verificationFailure();
  }
  const seen = new Set<string>();
  let achieved = 0;
  for (const attestation of certificate.attestations) {
    const guarantorId = exactBytes(attestation.guarantorId, 32);
    const identity = Array.from(guarantorId, (byte) => byte.toString(16).padStart(2, "0")).join("");
    if (
      seen.has(identity)
      || !equal(attestation.checkpointId, checkpointId)
      || !equal(attestation.checkpointHash, checkpointId)
      || attestation.batchNumber !== header.batchNumber
      || !equal(attestation.dataAvailabilityRoot, header.dataAvailabilityRoot)
      || !attestation.replayed
      || !attestation.dataPossessed
      || attestation.availabilityClassMask !== ALL_AVAILABILITY_CLASSES
      || attestation.attestedAtMs <= 0n
    ) {
      return verificationFailure();
    }
    seen.add(identity);
    const member = input.bondedSet.find((candidate) => candidate.bonded && equal(candidate.guarantorId, guarantorId));
    if (member === undefined) {
      return verificationFailure();
    }
    const digest = await sha256(CHECKPOINT_DOMAIN, attestationMessage(attestation));
    if (!await signatures.verifySecp256k1(
      exactBytes(member.publicKey, 33),
      exactBytes(attestation.signature, 64),
      digest,
    )) {
      return verificationFailure();
    }
    achieved += 1;
  }
  if (achieved < certificate.threshold) {
    return verificationFailure();
  }
  const settlement = certificate.settlementReference;
  if (settlement !== undefined && (
    settlement.length === 0
    || input.registeredSettlementReference === undefined
    || !equal(settlement, input.registeredSettlementReference)
  )) {
    return verificationFailure();
  }
  return Object.freeze({
    level: settlement === undefined ? "checkpoint-finalised" : "settlement-anchored",
    checkpointId,
    achieved,
    required: certificate.threshold,
    header,
  });
}
