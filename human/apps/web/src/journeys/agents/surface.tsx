"use client";

import { useRouter } from "next/navigation";
import { useCallback, useEffect, useMemo, useState } from "react";

import { copyEntry } from "../../../copy/catalog.ts";
import { useActiveAccountId } from "../../auth/use-active-account.ts";
import {
  Badge,
  KitButton,
  KitList,
  KitListItem,
  ScreenCard,
  StateEmpty,
} from "../../kit";
import { LoadingSurface } from "../../states";
import { PrivateFigure } from "../../settings";
import { AgentDetailScreen } from "./detail.tsx";
import {
  AGENT_LOCALE,
  Agents,
  agentListItems,
  agentsLayout,
  apiErrorSentence,
  type AgentListItemView,
  type AgentsShell,
} from "./model.ts";
import { useAgentsShell } from "./shell.ts";

function AgentList({
  items,
  onSelect,
  selected,
}: Readonly<{
  items: readonly AgentListItemView[];
  onSelect: (agentId: string) => void;
  selected?: string;
}>) {
  return (
    <KitList>
      {items.map((item) => (
        <KitListItem
          key={item.agentId}
          title={item.name}
          subtitle={<PrivateFigure>{item.spendSummary}</PrivateFigure>}
          trailing={<Badge variant={item.tone}>{item.stateLabel}</Badge>}
          trailingCaption={item.verificationSentence}
          navigates
          aria-current={item.agentId === selected ? "true" : undefined}
          onClick={() => {
            onSelect(item.agentId);
          }}
        />
      ))}
    </KitList>
  );
}

export function AgentsSurface({
  shell: initialShell,
  ownerAccount,
}: Readonly<{ shell: AgentsShell; ownerAccount?: string }>) {
  const router = useRouter();
  const shell = useAgentsShell(initialShell);
  const layout = agentsLayout(shell);
  const agents = useMemo(() => new Agents(), []);
  const accountId = useActiveAccountId(ownerAccount);
  const [items, setItems] = useState<readonly AgentListItemView[] | undefined>(undefined);
  const [selected, setSelected] = useState<string | undefined>(undefined);
  const [errorSentence, setErrorSentence] = useState<string | undefined>(undefined);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    setLoading(true);
    setErrorSentence(undefined);
    try {
      setItems(agentListItems(await agents.overview(), AGENT_LOCALE));
    } catch (error) {
      setErrorSentence(apiErrorSentence(error));
    } finally {
      setLoading(false);
    }
  }, [agents]);

  useEffect(() => {
    void load();
  }, [load]);

  const newAgent = (
    <KitButton
      variant="primary"
      onClick={() => {
        router.push("/app/agents/new");
      }}
    >
      {copyEntry("action.new_agent").message}
    </KitButton>
  );

  let body;
  if (loading && items === undefined) {
    body = <LoadingSurface />;
  } else if (items === undefined) {
    body = (
      <div className="flex flex-col gap-3">
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
      </div>
    );
  } else if (items.length === 0) {
    body = (
      <StateEmpty
        title={copyEntry("agents.empty").message}
        description={copyEntry("agents.empty.body").message}
        action={newAgent}
      />
    );
  } else if (layout === "stacked") {
    body = (
      <AgentList
        items={items}
        onSelect={(agentId) => {
          router.push(`/app/agents/${agentId}`);
        }}
      />
    );
  } else {
    body = (
      <div className="grid grid-cols-[minmax(280px,360px)_1fr] items-start gap-6">
        <AgentList
          items={items}
          {...(selected === undefined ? {} : { selected })}
          onSelect={setSelected}
        />
        {selected === undefined ? (
          <StateEmpty title={copyEntry("agent.list.select").message} />
        ) : (
          <AgentDetailScreen
            shell="desktop"
            agentId={selected}
            embedded
            {...(accountId === undefined ? {} : { ownerAccount: accountId })}
          />
        )}
      </div>
    );
  }

  return (
    <ScreenCard
      landmark="section"
      title={copyEntry("navigation.agents").message}
      description={copyEntry("agents.summary").message}
    >
      <div className="flex">{newAgent}</div>
      {body}
    </ScreenCard>
  );
}
