import { NextResponse } from "next/server";

import { verifyEvidenceUpstream } from "../../../../explorer/client";
import { encodeVerificationReport } from "../../../../explorer/model";

const MAXIMUM_BODY_BYTES = 1_100_000;
const MAXIMUM_EVIDENCE_CHARACTERS = 1_050_000;

async function readBoundedBody(request: Request): Promise<string | undefined> {
  if (request.body === null) {
    return undefined;
  }
  const reader = request.body.getReader();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let received = 0;
  let body = "";
  try {
    while (true) {
      const chunk = await reader.read();
      if (chunk.done) {
        body += decoder.decode();
        return body;
      }
      received += chunk.value.byteLength;
      if (received > MAXIMUM_BODY_BYTES) {
        await reader.cancel();
        return undefined;
      }
      body += decoder.decode(chunk.value, { stream: true });
    }
  } catch {
    return undefined;
  }
}

function requestBody(value: unknown): Readonly<{ kind: string; evidence: string }> | undefined {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return undefined;
  }
  const item = value as Readonly<Record<string, unknown>>;
  const kind = item.kind;
  const evidence = item.evidence;
  if (
    (kind !== "receipt" && kind !== "activity-inclusion" && kind !== "state-inclusion")
    || typeof evidence !== "string"
    || evidence.length === 0
    || evidence.length > MAXIMUM_EVIDENCE_CHARACTERS
  ) {
    return undefined;
  }
  return Object.freeze({ kind, evidence });
}

export async function POST(request: Request) {
  const contentLength = request.headers.get("content-length");
  if (contentLength !== null && Number(contentLength) > MAXIMUM_BODY_BYTES) {
    return NextResponse.json({ status: "invalid" }, { status: 413 });
  }
  const body = await readBoundedBody(request);
  if (body === undefined) {
    return NextResponse.json({ status: "invalid" }, { status: 413 });
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(body);
  } catch {
    return NextResponse.json({ status: "invalid" }, { status: 400 });
  }
  const valid = requestBody(parsed);
  if (valid === undefined) {
    return NextResponse.json({ status: "invalid" }, { status: 400 });
  }
  try {
    const report = await verifyEvidenceUpstream(valid);
    return NextResponse.json(encodeVerificationReport(report), {
      headers: { "Cache-Control": "no-store" },
    });
  } catch (error) {
    const unavailable = error instanceof Error && error.name === "ExplorerUnavailableError";
    return NextResponse.json(
      { status: unavailable ? "unavailable" : "refused" },
      { status: unavailable ? 503 : 422, headers: { "Cache-Control": "no-store" } },
    );
  }
}
