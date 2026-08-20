import { NextResponse } from "next/server";

import { RUM_SERIES_CAPACITY } from "../../../../perf/budgets";
import {
  parseRedactedWebVital,
  recordWebVital,
  rumSnapshot,
} from "../../../../perf/rum-store";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

const MAXIMUM_BODY_BYTES = 512;
const NO_STORE_HEADERS = { "cache-control": "no-store" } as const;

export async function POST(request: Request) {
  const fetchSite = request.headers.get("sec-fetch-site");
  if (fetchSite !== null && fetchSite !== "same-origin" && fetchSite !== "same-site") {
    return NextResponse.json({ accepted: false }, { status: 403, headers: NO_STORE_HEADERS });
  }
  const body = await request.text();
  if (new TextEncoder().encode(body).length > MAXIMUM_BODY_BYTES) {
    return NextResponse.json({ accepted: false }, { status: 413, headers: NO_STORE_HEADERS });
  }
  let decoded: unknown;
  try {
    decoded = JSON.parse(body) as unknown;
  } catch {
    return NextResponse.json({ accepted: false }, { status: 400, headers: NO_STORE_HEADERS });
  }
  const observation = parseRedactedWebVital(decoded);
  if (observation === undefined) {
    return NextResponse.json({ accepted: false }, { status: 400, headers: NO_STORE_HEADERS });
  }
  recordWebVital(observation);
  return NextResponse.json({ accepted: true }, { status: 202, headers: NO_STORE_HEADERS });
}

export function GET() {
  return NextResponse.json(
    { version: 1, capacityPerSeries: RUM_SERIES_CAPACITY, aggregates: rumSnapshot() },
    { headers: NO_STORE_HEADERS },
  );
}
