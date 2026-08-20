import { createHash, randomUUID, timingSafeEqual } from "node:crypto";
import { link, mkdir, open, readFile, readdir, stat, unlink } from "node:fs/promises";
import { isAbsolute, join, resolve } from "node:path";

import {
  parseStoredSupportReport,
  supportReportTrace,
  type StoredSupportReport,
  type SupportReportRequest,
} from "../states/report-schema.ts";

const REPORT_FILE = /^[a-f0-9]{64}\.json$/u;
const DEFAULT_RETENTION = 1_000;
const MAX_RETENTION = 10_000;

export class SupportReportConfigurationError extends Error {}
export class SupportReportConflictError extends Error {}

function nodeError(error: unknown): error is NodeJS.ErrnoException {
  return error instanceof Error && "code" in error;
}

function reportId(traceId: string): string {
  return createHash("sha256").update(traceId, "utf8").digest("hex");
}

function retentionFromEnvironment(): number {
  const configured = process.env.LAYERX_SUPPORT_REPORT_RETENTION;
  if (configured === undefined || configured.trim() === "") {
    return DEFAULT_RETENTION;
  }
  const parsed = Number(configured);
  if (!Number.isSafeInteger(parsed) || parsed < 1 || parsed > MAX_RETENTION) {
    throw new SupportReportConfigurationError("Support report retention is invalid");
  }
  return parsed;
}

function storageDirectoryFromEnvironment(): string {
  const configured = process.env.LAYERX_SUPPORT_REPORT_DIR?.trim();
  if (configured === undefined || configured === "" || !isAbsolute(configured)) {
    throw new SupportReportConfigurationError("Support report storage directory is not configured");
  }
  const directory = resolve(configured);
  if (directory === "/") {
    throw new SupportReportConfigurationError("Support report storage directory is too broad");
  }
  return directory;
}

async function syncDirectory(directory: string): Promise<void> {
  const handle = await open(directory, "r");
  try {
    await handle.sync();
  } finally {
    await handle.close();
  }
}

function sameStoredReport(stored: StoredSupportReport, report: SupportReportRequest): boolean {
  return stored.traceId === report.traceId
    && stored.machineCode === report.machineCode
    && stored.route === report.route
    && stored.shell === report.shell;
}

export class SupportReportRepository {
  readonly directory: string;
  readonly retention: number;

  constructor(directory: string, retention: number) {
    if (!isAbsolute(directory) || resolve(directory) === "/") {
      throw new SupportReportConfigurationError("Support report storage directory is invalid");
    }
    if (!Number.isSafeInteger(retention) || retention < 1 || retention > MAX_RETENTION) {
      throw new SupportReportConfigurationError("Support report retention is invalid");
    }
    this.directory = directory;
    this.retention = retention;
  }

  async save(report: SupportReportRequest): Promise<Readonly<{ record: StoredSupportReport; created: boolean }>> {
    await mkdir(this.directory, { recursive: true, mode: 0o700 });
    const identifier = reportId(report.traceId);
    const record = Object.freeze({
      ...report,
      reportId: identifier,
      receivedAt: new Date().toISOString(),
    });
    const destination = join(this.directory, `${identifier}.json`);
    const temporary = join(this.directory, `.pending-${identifier}-${randomUUID()}`);
    const handle = await open(temporary, "wx", 0o600);
    try {
      await handle.writeFile(`${JSON.stringify(record)}\n`, "utf8");
      await handle.sync();
    } finally {
      await handle.close();
    }

    try {
      await link(temporary, destination);
      await syncDirectory(this.directory);
    } catch (error) {
      if (!nodeError(error) || error.code !== "EEXIST") {
        throw error;
      }
      const existing = await this.findByTrace(report.traceId);
      if (existing === undefined || !sameStoredReport(existing, report)) {
        throw new SupportReportConflictError("Trace already belongs to a different report");
      }
      await this.#enforceRetention();
      return Object.freeze({ record: existing, created: false });
    } finally {
      await unlink(temporary).catch((error: unknown) => {
        if (!nodeError(error) || error.code !== "ENOENT") {
          throw error;
        }
      });
    }

    await this.#enforceRetention(`${identifier}.json`);
    return Object.freeze({ record, created: true });
  }

  async findByTrace(traceId: string): Promise<StoredSupportReport | undefined> {
    const normalized = supportReportTrace(traceId);
    const location = join(this.directory, `${reportId(normalized)}.json`);
    try {
      return parseStoredSupportReport(JSON.parse(await readFile(location, "utf8")));
    } catch (error) {
      if (nodeError(error) && error.code === "ENOENT") {
        return undefined;
      }
      throw error;
    }
  }

  async #enforceRetention(protectedFile?: string): Promise<void> {
    const entries = (await readdir(this.directory, { withFileTypes: true }))
      .filter((entry) => entry.isFile() && REPORT_FILE.test(entry.name));
    if (entries.length <= this.retention) {
      return;
    }
    const aged = await Promise.all(entries.map(async (entry) => ({
      name: entry.name,
      modifiedAt: (await stat(join(this.directory, entry.name))).mtimeMs,
    })));
    aged.sort((left, right) => left.modifiedAt - right.modifiedAt || left.name.localeCompare(right.name));
    const expiredReports = aged
      .filter((entry) => entry.name !== protectedFile)
      .slice(0, aged.length - this.retention);
    for (const expired of expiredReports) {
      await unlink(join(this.directory, expired.name)).catch((error: unknown) => {
        if (!nodeError(error) || error.code !== "ENOENT") {
          throw error;
        }
      });
    }
    await syncDirectory(this.directory);
  }
}

export function supportReportRepositoryFromEnvironment(): SupportReportRepository {
  return new SupportReportRepository(storageDirectoryFromEnvironment(), retentionFromEnvironment());
}

export function supportRetrievalAuthorized(request: Request): boolean {
  const credential = process.env.LAYERX_SUPPORT_REPORT_BEARER_TOKEN?.trim();
  if (credential === undefined || credential.length < 32) {
    throw new SupportReportConfigurationError("Support report retrieval credential is not configured");
  }
  const supplied = request.headers.get("authorization") ?? "";
  const expectedDigest = createHash("sha256").update(`Bearer ${credential}`, "utf8").digest();
  const suppliedDigest = createHash("sha256").update(supplied, "utf8").digest();
  return timingSafeEqual(expectedDigest, suppliedDigest);
}
