import { copyEntry } from "../../../copy/catalog.ts";
import { formatCopy } from "../../../copy/format.ts";
import {
  type ActivityEntryDetail,
  type Agent,
  type ApprovalSummary,
  type HumanApiClient,
  type VerifiedMoney,
} from "../../api/index.ts";
import {
  directionWord,
  statusKeyFromCopyKey,
  verificationWord,
  type ProtocolAmount,
  type StatusKey,
} from "../../kit/model.ts";
import {
  MOVE_ROUTE,
  moneyAmountNumber,
  moneyWords,
  moveFailure,
  timestampWords,
  verifiedOutcomeEvidence,
} from "../move/model.ts";

export const HOME_ACTIVITY_LIMIT = 5;

export const HOME_DESTINATIONS = Object.freeze({
  add: "/app/deposit",
  move: MOVE_ROUTE,
  withdraw: "/app/withdraw",
  approvals: "/app/approvals",
  agents: "/app/agents",
  activity: "/app/activity",
});

export interface HomeActionItem {
  readonly id: keyof typeof HOME_DESTINATIONS & ("add" | "move" | "withdraw");
  readonly label: string;
  readonly icon: "add" | "move" | "withdraw";
  readonly route: string;
}

export function homeActions(): readonly [HomeActionItem, HomeActionItem, HomeActionItem] {
  return [
    { id: "add", label: copyEntry("home.actions.add").message, icon: "add", route: HOME_DESTINATIONS.add },
    { id: "move", label: copyEntry("home.actions.move").message, icon: "move", route: HOME_DESTINATIONS.move },
    {
      id: "withdraw",
      label: copyEntry("home.actions.withdraw").message,
      icon: "withdraw",
      route: HOME_DESTINATIONS.withdraw,
    },
  ];
}

export interface HomeData {
  readonly agents: readonly Agent[];
  readonly approvals: readonly ApprovalSummary[];
  readonly entries: readonly ActivityEntryDetail[];
}

export async function loadHome(client: HumanApiClient): Promise<HomeData> {
  const [agentPage, approvalPage, activityPage] = await Promise.all([
    client.agentList(),
    client.approvalList(),
    client.activityQuery({ page_limit: HOME_ACTIVITY_LIMIT }),
  ]);
  const summaries = activityPage.groups
    .flatMap((group) => group.entries)
    .slice(0, HOME_ACTIVITY_LIMIT);
  const entries = await Promise.all(
    summaries.map((entry) => client.activityEntry(entry.entry_id)),
  );
  return {
    agents: agentPage.agents,
    approvals: approvalPage.approvals,
    entries,
  };
}

export type HomeLoadFailure = "offline" | "error";

export function classifyHomeFailure(error: unknown): HomeLoadFailure {
  return moveFailure(error).kind === "offline" ? "offline" : "error";
}

export type HomeBalance =
  | Readonly<{
      kind: "verified";
      label: string;
      amount: ProtocolAmount;
      currency: string;
      verification: string;
      freshness: string;
    }>
  | Readonly<{ kind: "unavailable"; label: string; message: string }>;

export function homeBalance(verified?: VerifiedMoney, checkedAgo?: string): HomeBalance {
  const label = copyEntry("home.balance.label").message;
  if (verified === undefined || checkedAgo === undefined) {
    return { kind: "unavailable", label, message: copyEntry("home.balance.unavailable").message };
  }
  return {
    kind: "verified",
    label,
    amount: moneyAmountNumber(verified.money),
    currency: verified.money.currency,
    verification: verificationWord(verified.verification),
    freshness: formatCopy("home.balance.freshness", { when: checkedAgo }),
  };
}

export function approvalBadge(
  approvals: readonly ApprovalSummary[],
): Readonly<{ count: number; label: string }> {
  const count = approvals.filter((approval) => approval.state === "pending").length;
  return { count, label: formatCopy("approval.count", { count }) };
}

export interface HomeAgentRow {
  readonly id: string;
  readonly name: string;
  readonly purpose: string;
  readonly active: boolean;
  readonly spend: string;
  readonly spendVerification: string;
}

export function homeAgentRows(agents: readonly Agent[]): readonly HomeAgentRow[] {
  return agents.map((agent) => ({
    id: agent.agent_id,
    name: agent.name,
    purpose: agent.purpose,
    active: agent.state === "active",
    spend: formatCopy("home.agents.spent", {
      spent: moneyWords(agent.spend.spent),
      limit: moneyWords(agent.limit.monthly),
    }),
    spendVerification: verificationWord(agent.spend.verification),
  }));
}

export function agentsSummary(agents: readonly Agent[]): string {
  return formatCopy("home.agents.count", { count: agents.length });
}

export interface HomeActivityRow {
  readonly id: string;
  readonly title: string;
  readonly status: StatusKey;
  readonly when: string;
  readonly amount?: ProtocolAmount;
  readonly currency?: string;
  readonly direction?: "inbound" | "outbound" | "other";
}

function activityTitle(entry: ActivityEntryDetail): string {
  try {
    return copyEntry(entry.summary_copy_key).message;
  } catch {
    return directionWord(
      entry.direction === "in" ? "inbound" : entry.direction === "out" ? "outbound" : "other",
    );
  }
}

export function homeActivityRows(entries: readonly ActivityEntryDetail[]): readonly HomeActivityRow[] {
  return entries.map((entry) => {
    const outcomeBacked = [...entry.evidence, ...entry.stages.flatMap((stage) => stage.evidence)]
      .some(verifiedOutcomeEvidence);
    const receiptRequired = entry.kind !== "security-event";
    const row: {
      id: string;
      title: string;
      status: StatusKey;
      when: string;
      amount?: ProtocolAmount;
      currency?: string;
      direction?: "inbound" | "outbound" | "other";
    } = {
      id: entry.entry_id,
      title: activityTitle(entry),
      status:
        (entry.state === "done" || entry.state === "done-finalised") &&
        receiptRequired &&
        !outcomeBacked
          ? "processing"
          : statusKeyFromCopyKey(entry.state),
      when: timestampWords(entry.occurred_at),
    };
    if (entry.money !== undefined) {
      row.amount = moneyAmountNumber(entry.money);
      row.currency = entry.money.currency;
      row.direction =
        entry.direction === "in" ? "inbound" : entry.direction === "out" ? "outbound" : "other";
    }
    return row;
  });
}
