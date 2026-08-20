"use client";

import { useRouter } from "next/navigation";
import { useCallback, useEffect, useMemo, useState } from "react";

import { copyEntry } from "../../../copy/catalog.ts";
import type { Agent } from "../../api/index.ts";
import { useActiveAccountId } from "../../auth/use-active-account.ts";
import {
  Badge,
  CopyableIdentifier,
  InlineNotice,
  KitButton,
  KitSectionHeader,
  LabelValue,
  ScreenCard,
  StatPair,
} from "../../kit";
import { LoadingSurface } from "../../states";
import { PrivateFigure } from "../../settings";
import { AgentControls } from "./controls.tsx";
import {
  AGENT_LOCALE,
  Agents,
  agentPresentation,
  apiErrorSentence,
  creationHeadlineKey,
  formatPlainTimestamp,
  journeyProgress,
  spendPresentation,
  type AgentsShell,
  type JourneyProgress,
} from "./model.ts";
import { JourneyStages } from "./progress.tsx";
import { useAgentsShell } from "./shell.ts";

function SpendSection({ agent }: Readonly<{ agent: Agent }>) {
  const spend = spendPresentation(agent, AGENT_LOCALE);
  return (
    <div className="flex flex-col gap-3">
      <StatPair
        left={{
          value: <PrivateFigure>{spend.spent}</PrivateFigure>,
          label: copyEntry("agent.spend.spent").message,
        }}
        right={{
          value: <PrivateFigure>{spend.remaining}</PrivateFigure>,
          label: copyEntry("agent.spend.remaining").message,
        }}
      />
      <LabelValue
        label={copyEntry("agent.spend.limit").message}
        value={<PrivateFigure>{spend.limit}</PrivateFigure>}
      />
      <p className="text-sm text-foreground-secondary">
        <PrivateFigure>{spend.summary}</PrivateFigure>
      </p>
      <InlineNotice tone={spend.protocolBacked ? "neutral" : "warning"} role="status">
        {spend.enforcementSentence}
      </InlineNotice>
      <p className="text-sm text-muted-foreground">{spend.verificationSentence}</p>
      {spend.reconciliationSentence === undefined ? null : (
        <InlineNotice tone="warning" role="status">
          {spend.reconciliationSentence}
        </InlineNotice>
      )}
    </div>
  );
}

export function AgentDetailScreen({
  shell: initialShell,
  agentId,
  ownerAccount,
  embedded = false,
}: Readonly<{
  shell: AgentsShell;
  agentId: string;
  ownerAccount?: string;
  embedded?: boolean;
}>) {
  const router = useRouter();
  const shell = useAgentsShell(initialShell);
  const agents = useMemo(() => new Agents(), []);
  const accountId = useActiveAccountId(ownerAccount);
  const [agent, setAgent] = useState<Agent | undefined>(undefined);
  const [creation, setCreation] = useState<JourneyProgress | undefined>(undefined);
  const [errorSentence, setErrorSentence] = useState<string | undefined>(undefined);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    setLoading(true);
    setErrorSentence(undefined);
    try {
      const loaded = await agents.agent(agentId);
      setAgent(loaded);
      if (loaded.state === "creating" && loaded.creation_journey_id !== undefined) {
        setCreation(journeyProgress(await agents.journey(loaded.creation_journey_id)));
      } else {
        setCreation(undefined);
      }
    } catch (error) {
      setErrorSentence(apiErrorSentence(error));
    } finally {
      setLoading(false);
    }
  }, [agentId, agents]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (creation === undefined || creation.complete || creation.refusalSentence !== undefined) {
      return;
    }
    const timer = setTimeout(() => {
      void load();
    }, 2_000);
    return () => {
      clearTimeout(timer);
    };
  }, [creation, load]);

  if (loading && agent === undefined) {
    return <LoadingSurface />;
  }
  if (agent === undefined) {
    return (
      <ScreenCard landmark="section" title={copyEntry("state.error").message}>
        <p className="text-sm text-foreground-secondary">
          {errorSentence ?? copyEntry("state.error.body").message}
        </p>
        <div className="flex">
          <KitButton
            variant="primary"
            onClick={() => {
              void load();
            }}
          >
            {copyEntry("action.retry").message}
          </KitButton>
        </div>
      </ScreenCard>
    );
  }

  const presentation = agentPresentation(agent);
  return (
    <ScreenCard landmark="section" title={agent.name}>
      <div className="flex items-center gap-3">
        <Badge variant={presentation.tone}>{presentation.label}</Badge>
      </div>
      {presentation.stateVerified ? null : (
        <InlineNotice tone="warning" role="status">
          {copyEntry("agent.state.unverified").message}
        </InlineNotice>
      )}
      {presentation.readOnlyKey === undefined ? null : (
        <InlineNotice tone="neutral" role="status">
          {copyEntry(presentation.readOnlyKey).message}
        </InlineNotice>
      )}
      <div className="flex flex-col gap-2">
        <LabelValue label={copyEntry("agent.detail.purpose").message} value={agent.purpose} />
        <LabelValue
          label={copyEntry("agent.detail.created").message}
          value={formatPlainTimestamp(agent.created_at, AGENT_LOCALE)}
        />
      </div>
      <CopyableIdentifier
        label={copyEntry("agent.detail.identifier").message}
        value={agent.agent_id}
      />
      {creation === undefined ? null : (
        <div className="flex flex-col gap-3">
          <KitSectionHeader title={copyEntry("agent.detail.progress").message} />
          <p className="text-sm text-foreground-secondary">
            {copyEntry(creationHeadlineKey(creation)).message}
          </p>
          <JourneyStages progress={creation} />
        </div>
      )}
      <SpendSection agent={agent} />
      <AgentControls
        shell={shell}
        agent={agent}
        agents={agents}
        {...(accountId === undefined ? {} : { ownerAccount: accountId })}
        onAgent={setAgent}
        onChanged={() => {
          void load();
        }}
      />
      {embedded ? null : (
        <div className="flex">
          <KitButton
            variant="secondary"
            onClick={() => {
              router.push("/app/agents");
            }}
          >
            {copyEntry("action.back_to_agents").message}
          </KitButton>
        </div>
      )}
    </ScreenCard>
  );
}
