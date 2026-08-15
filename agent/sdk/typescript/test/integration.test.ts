import {
  VerificationLevel,
  layerx_sdk_ts_package,
  parseAmount,
  parseSequence,
  requireVerified,
  type SubmissionState,
  type VerifiedRead,
} from "../src/index.js";

function assert(condition: boolean, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

function expectThrow(operation: () => unknown, message: string): void {
  let threw = false;
  try {
    operation();
  } catch {
    threw = true;
  }
  assert(threw, message);
}

export function verifyTypeScriptPackage(): void {
  const metadata = layerx_sdk_ts_package();
  assert(metadata.contractMajor === 1, "contract major drift");
  assert(parseAmount("9007199254740993") === 9007199254740993n, "amount lost precision");
  assert(parseSequence("18446744073709551615") === 18446744073709551615n, "sequence lost precision");
  expectThrow(() => parseAmount("1.5"), "fractional amount accepted");
  expectThrow(() => parseSequence("18446744073709551616"), "overflowing sequence accepted");

  const read: VerifiedRead<bigint> = {
    value: 10n,
    achievedVerificationLevel: VerificationLevel.StateProven,
    freshness: {
      chainHead: 20n,
      latestBatch: "batch-19",
      latestCheckpoint: "checkpoint-18",
      valueSequence: 17n,
    },
  };
  assert(requireVerified(VerificationLevel.StateProven, read) === read, "proven read changed");
  expectThrow(
    () => requireVerified(VerificationLevel.CheckpointFinalised, read),
    "below-requested read accepted",
  );

  const unknown: SubmissionState = { kind: "Unknown" };
  const futureFailure: SubmissionState = { kind: "Failed", protocolResultCode: -77777 };
  assert(unknown.kind === "Unknown", "unknown collapsed");
  assert(futureFailure.protocolResultCode === -77777, "protocol code changed");
}

verifyTypeScriptPackage();
