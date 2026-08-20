import {
  HumanApiError,
  type ActivityEntryDetail,
  type ApprovalDetail,
  type ApprovalSummary,
  type HumanApiClient,
  type StepUpEvidence,
} from "../../api/index.ts";
import { performStepUp, type PasskeyAuthenticator } from "./ceremony.ts";
import {
  decidedOutcome,
  convergedOutcome,
  decisionKey,
  defectiveOutcome,
  failureOutcome,
  heldDigest,
  releasedActivity,
  stepUpEvidenceReference,
  type ApprovalOutcome,
} from "./model.ts";

export interface ApprovalsOptions {
  readonly client: HumanApiClient;
  readonly authenticator: PasskeyAuthenticator;
}

export class Approvals {
  readonly #client: HumanApiClient;
  readonly #authenticator: PasskeyAuthenticator;

  constructor(options: ApprovalsOptions) {
    this.#client = options.client;
    this.#authenticator = options.authenticator;
  }

  async inbox(): Promise<ApprovalSummary[]> {
    const page = await this.#client.approvalList();
    return page.approvals;
  }

  async detail(approvalId: string): Promise<ApprovalDetail> {
    return this.#client.approvalGet(approvalId);
  }

  async approve(detail: ApprovalDetail, key: string = decisionKey()): Promise<ApprovalOutcome> {
    const digest = heldDigest(detail);
    if (digest === undefined) {
      return defectiveOutcome();
    }
    let evidence: StepUpEvidence;
    try {
      evidence = await performStepUp(this.#client, digest, this.#authenticator);
    } catch (error) {
      if (error instanceof HumanApiError) {
        return this.#failure(error);
      }
      throw error;
    }
    try {
      const decision = await this.#client.approvalApprove(
        detail.approval_id,
        { step_up_evidence: stepUpEvidenceReference(evidence) },
        key,
      );
      return decidedOutcome(decision);
    } catch (error) {
      if (error instanceof HumanApiError) {
        const failure = this.#failure(error);
        if (failure.kind !== "already-decided") {
          return failure;
        }
      }
      return this.resolve(detail.approval_id);
    }
  }

  async reject(approvalId: string, key: string = decisionKey()): Promise<ApprovalOutcome> {
    try {
      const decision = await this.#client.approvalReject(approvalId, key);
      return decidedOutcome(decision);
    } catch (error) {
      if (error instanceof HumanApiError) {
        const failure = this.#failure(error);
        if (failure.kind !== "already-decided") {
          return failure;
        }
      }
      return this.resolve(approvalId);
    }
  }

  async resolve(approvalId: string): Promise<ApprovalOutcome> {
    return convergedOutcome(await this.#client.approvalGet(approvalId));
  }

  async released(approvalId: string): Promise<ActivityEntryDetail | undefined> {
    const page = await this.#client.activityQuery({});
    const entry = releasedActivity(page, approvalId);
    return entry === undefined ? undefined : this.#client.activityEntry(entry.entry_id);
  }

  #failure(error: HumanApiError): ApprovalOutcome {
    const outcome = failureOutcome(error.detail);
    if (outcome === undefined) {
      throw error;
    }
    return outcome;
  }
}
