"use client";

import { useEffect, useState } from "react";

import { copyEntry } from "../../../copy/catalog.ts";
import type { Agent, MoveQuote } from "../../api/index.ts";
import {
  DesktopConfirmation,
  InlineNotice,
  KitButton,
  KitSectionHeader,
  KitTextField,
  MobileConfirmation,
} from "../../kit";
import { PrivateFigure } from "../../settings";
import {
  AGENT_CURRENCY,
  AGENT_LOCALE,
  Agents,
  apiErrorCode,
  apiErrorSentence,
  controlsFor,
  journeyProgress,
  keyChallengePresentation,
  mutationOutcomeUnknown,
  parseMonthlyLimit,
  quotePresentation,
  type AgentControl,
  type AgentsShell,
  type JourneyProgress,
  type KeyChallengePresentation,
} from "./model.ts";
import { JourneyStages } from "./progress.tsx";

type OpenControl =
  | Readonly<{ id: "pause" | "resume" | "rotate" | "recover" }>
  | Readonly<{ id: "limit"; input: string }>
  | Readonly<{ id: "reclaim"; input: string }>
  | Readonly<{ id: "fund"; input: string; quote?: MoveQuote }>
  | Readonly<{ id: "archive"; phase: "disposition" | "confirm"; typed: string }>;

function AmountField({
  labelKey,
  value,
  onChange,
}: Readonly<{ labelKey: string; value: string; onChange: (value: string) => void }>) {
  return (
    <KitTextField
      label={copyEntry(labelKey).message}
      value={value}
      onChange={(event) => {
        onChange(event.target.value);
      }}
      autoComplete="off"
      spellCheck={false}
      inputMode="numeric"
    />
  );
}

export function AgentControls({
  shell,
  agent,
  agents,
  ownerAccount,
  onAgent,
  onChanged,
}: Readonly<{
  shell: AgentsShell;
  agent: Agent;
  agents: Agents;
  ownerAccount?: string;
  onAgent: (agent: Agent) => void;
  onChanged: () => void;
}>) {
  const [open, setOpen] = useState<OpenControl | undefined>(undefined);
  const [busy, setBusy] = useState(false);
  const [errorSentence, setErrorSentence] = useState<string | undefined>(undefined);
  const [lastJourney, setLastJourney] = useState<JourneyProgress | undefined>(undefined);
  const [challenge, setChallenge] = useState<KeyChallengePresentation | undefined>(undefined);
  const [outcomeUnknown, setOutcomeUnknown] = useState(false);

  const controls = controlsFor(agent, ownerAccount === undefined ? {} : { ownerAccount });
  const Confirmation = shell === "mobile" ? MobileConfirmation : DesktopConfirmation;
  const journeyPending = lastJourney !== undefined
    && !lastJourney.complete
    && lastJourney.refusalSentence === undefined;
  const controlsLocked = journeyPending || outcomeUnknown;

  useEffect(() => {
    if (!journeyPending || lastJourney === undefined) {
      return;
    }
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const refresh = async () => {
      try {
        const next = journeyProgress(await agents.journey(lastJourney.journeyId));
        if (!cancelled) {
          setLastJourney(next);
          setErrorSentence(undefined);
        }
      } catch (error) {
        if (!cancelled) {
          setErrorSentence(apiErrorSentence(error));
          timer = setTimeout(() => {
            void refresh();
          }, 2_000);
        }
      }
    };
    timer = setTimeout(() => {
      void refresh();
    }, 2_000);
    return () => {
      cancelled = true;
      if (timer !== undefined) {
        clearTimeout(timer);
      }
    };
  }, [agents, journeyPending, lastJourney]);

  if (
    controls.length === 0
    && lastJourney === undefined
    && challenge === undefined
    && errorSentence === undefined
  ) {
    return null;
  }

  const close = () => {
    setOpen(undefined);
    setErrorSentence(undefined);
    setOutcomeUnknown(false);
  };

  const openControl = (control: AgentControl) => {
    setErrorSentence(undefined);
    if (control.id === "limit" || control.id === "reclaim" || control.id === "fund") {
      setOpen({ id: control.id, input: "" });
    } else if (control.id === "archive") {
      setOpen({ id: "archive", phase: "disposition", typed: "" });
    } else {
      setOpen({ id: control.id });
    }
  };

  const confirm = async () => {
    if (open === undefined || busy) {
      return;
    }
    setBusy(true);
    setErrorSentence(undefined);
    try {
      setOutcomeUnknown(false);
      if (open.id === "pause") {
        onAgent(await agents.pause(agent.agent_id));
        close();
      } else if (open.id === "resume") {
        onAgent(await agents.resume(agent.agent_id));
        close();
      } else if (open.id === "limit") {
        const money = parseMonthlyLimit(open.input, AGENT_CURRENCY);
        if (money === undefined) {
          setErrorSentence(copyEntry("error.agent.limit-invalid").message);
        } else {
          onAgent(await agents.changeLimit(agent.agent_id, money));
          close();
        }
      } else if (open.id === "reclaim") {
        const money = parseMonthlyLimit(open.input, AGENT_CURRENCY);
        if (money === undefined) {
          setErrorSentence(copyEntry("error.agent.limit-invalid").message);
        } else {
          setLastJourney(journeyProgress(await agents.reclaim(agent.agent_id, money)));
          close();
          onChanged();
        }
      } else if (open.id === "fund") {
        if (ownerAccount === undefined) {
          setErrorSentence(copyEntry("agent.fund.unavailable").message);
        } else if (open.quote === undefined) {
          const money = parseMonthlyLimit(open.input, AGENT_CURRENCY);
          if (money === undefined) {
            setErrorSentence(copyEntry("error.agent.limit-invalid").message);
          } else {
            const quote = await agents.fundQuote(ownerAccount, agent.agent_id, money);
            setOpen({ id: "fund", input: open.input, quote });
          }
        } else {
          setLastJourney(journeyProgress(await agents.fundCommit(open.quote.quote_id)));
          close();
          onChanged();
        }
      } else if (open.id === "archive") {
        if (open.phase === "disposition") {
          setOpen({ id: "archive", phase: "confirm", typed: "" });
        } else {
          setLastJourney(journeyProgress(await agents.archive(agent.agent_id, open.typed)));
          close();
          onChanged();
        }
      } else if (open.id === "rotate") {
        setChallenge(keyChallengePresentation(await agents.rotate(agent.agent_id), AGENT_LOCALE));
        close();
      } else {
        setChallenge(keyChallengePresentation(await agents.recover(agent.agent_id), AGENT_LOCALE));
        close();
      }
    } catch (error) {
      const quoteReadFailed = open.id === "fund" && open.quote === undefined;
      if (mutationOutcomeUnknown(error) && !quoteReadFailed) {
        setOpen(undefined);
        setOutcomeUnknown(true);
        setErrorSentence(copyEntry("state.still_checking.body").message);
        return;
      }
      if (apiErrorCode(error) === "archive-needs-disposition") {
        setOpen({ id: "archive", phase: "disposition", typed: "" });
      }
      setErrorSentence(apiErrorSentence(error));
    } finally {
      setBusy(false);
    }
  };

  const errorNotice = errorSentence === undefined ? null : (
    <InlineNotice tone="danger" role="alert">
      {errorSentence}
    </InlineNotice>
  );

  const lifecycleControls = controls.filter(
    (control) => control.id !== "rotate" && control.id !== "recover",
  );
  const keyControls = controls.filter(
    (control) => control.id === "rotate" || control.id === "recover",
  );

  return (
    <div className="flex flex-col gap-4">
      {controls.length === 0 ? null : (
        <>
          <KitSectionHeader title={copyEntry("agent.detail.controls").message} />
          <div className="flex flex-wrap gap-2">
            {lifecycleControls.map((control) =>
              control.enabled && !controlsLocked ? (
                <KitButton
                  key={control.id}
                  variant={control.kind === "irreversible" ? "destructive" : "secondary"}
                  onClick={() => {
                    openControl(control);
                  }}
                >
                  {copyEntry(control.labelKey).message}
                </KitButton>
              ) : (
                <KitButton
                  key={control.id}
                  variant="secondary"
                  disabled
                  disabledReason={copyEntry(
                    controlsLocked ? "state.still_checking.locked" : control.disabledReasonKey ?? "state.error.body",
                  ).message}
                >
                  {copyEntry(control.labelKey).message}
                </KitButton>
              ),
            )}
          </div>
          <KitSectionHeader title={copyEntry("agent.detail.keys").message} />
          <div className="flex flex-wrap gap-2">
            {keyControls.map((control) => controlsLocked ? (
              <KitButton
                key={control.id}
                variant="secondary"
                disabled
                disabledReason={copyEntry("state.still_checking.locked").message}
              >
                {copyEntry(control.labelKey).message}
              </KitButton>
            ) : (
              <KitButton
                key={control.id}
                variant="secondary"
                onClick={() => {
                  openControl(control);
                }}
              >
                {copyEntry(control.labelKey).message}
              </KitButton>
            ))}
          </div>
        </>
      )}
      {open === undefined && errorSentence !== undefined ? (
        <InlineNotice tone={outcomeUnknown ? "warning" : "danger"} role="status">
          {errorSentence}
        </InlineNotice>
      ) : null}
      {challenge === undefined ? null : (
        <InlineNotice tone="neutral" role="status">
          <span className="flex flex-col gap-1">
            <span className="font-semibold">{challenge.startedSentence}</span>
            <span>{challenge.delaySentence}</span>
            <span>{challenge.readySentence}</span>
            <span>{copyEntry(challenge.bodyKey).message}</span>
          </span>
        </InlineNotice>
      )}
      {lastJourney === undefined ? null : <JourneyStages progress={lastJourney} />}
      {open === undefined ? null : open.id === "archive" && open.phase === "confirm" ? (
        <Confirmation
          open
          onOpenChange={(value) => {
            if (!value) {
              close();
            }
          }}
          kind="irreversible"
          title={copyEntry("agent.control.archive").message}
          consequence={copyEntry("agent.archive.consequence").message}
          confirmLabel={copyEntry("agent.control.archive").message}
          loading={busy}
          onConfirm={() => {
            void confirm();
          }}
          typedConfirmation={{
            expectedValue: agent.name,
            value: open.typed,
            onValueChange: (value) => {
              setOpen({ id: "archive", phase: "confirm", typed: value });
            },
          }}
        >
          {errorNotice}
        </Confirmation>
      ) : (
        <Confirmation
          open
          onOpenChange={(value) => {
            if (!value) {
              close();
            }
          }}
          kind={open.id === "archive" ? "destructive" : "reversible"}
          title={copyEntry(dialogTitleKey(open)).message}
          consequence={copyEntry(dialogConsequenceKey(open)).message}
          confirmLabel={copyEntry(dialogConfirmKey(open)).message}
          loading={busy}
          onConfirm={() => {
            void confirm();
          }}
        >
          <div className="flex flex-col gap-3">
            {open.id === "limit" ? (
              <AmountField
                labelKey="agent.limit.amount.label"
                value={open.input}
                onChange={(value) => {
                  setOpen({ id: "limit", input: value });
                }}
              />
            ) : null}
            {open.id === "reclaim" ? (
              <AmountField
                labelKey="agent.reclaim.amount.label"
                value={open.input}
                onChange={(value) => {
                  setOpen({ id: "reclaim", input: value });
                }}
              />
            ) : null}
            {open.id === "fund" && open.quote === undefined ? (
              <AmountField
                labelKey="agent.fund.amount.label"
                value={open.input}
                onChange={(value) => {
                  setOpen({ id: "fund", input: value });
                }}
              />
            ) : null}
            {open.id === "fund" && open.quote !== undefined ? (
              <QuoteSummary quote={open.quote} />
            ) : null}
            {errorNotice}
          </div>
        </Confirmation>
      )}
    </div>
  );
}

function QuoteSummary({ quote }: Readonly<{ quote: MoveQuote }>) {
  const presentation = quotePresentation(quote, AGENT_LOCALE);
  return (
    <div className="flex flex-col gap-1 text-sm text-foreground">
      <span className="font-semibold">{presentation.description}</span>
      <PrivateFigure className="tabular-nums">{presentation.amount}</PrivateFigure>
      <PrivateFigure>{presentation.feeSentence}</PrivateFigure>
      <span>{presentation.arrivalSentence}</span>
    </div>
  );
}

function dialogTitleKey(open: OpenControl): string {
  if (open.id === "archive") {
    return "agent.control.archive";
  }
  if (open.id === "fund") {
    return "agent.control.fund";
  }
  if (open.id === "reclaim") {
    return "agent.control.reclaim";
  }
  if (open.id === "limit") {
    return "agent.control.limit";
  }
  if (open.id === "pause") {
    return "agent.control.pause";
  }
  if (open.id === "resume") {
    return "agent.control.resume";
  }
  return open.id === "rotate" ? "agent.control.rotate" : "agent.control.recover";
}

function dialogConsequenceKey(open: OpenControl): string {
  if (open.id === "archive") {
    return "agent.archive.disposition";
  }
  if (open.id === "fund") {
    return "agent.fund.consequence";
  }
  if (open.id === "reclaim") {
    return "agent.reclaim.consequence";
  }
  if (open.id === "limit") {
    return "agent.limit.consequence";
  }
  if (open.id === "pause") {
    return "agent.pause.consequence";
  }
  if (open.id === "resume") {
    return "agent.resume.consequence";
  }
  return open.id === "rotate" ? "agent.keys.rotate.body" : "agent.keys.recover.body";
}

function dialogConfirmKey(open: OpenControl): string {
  if (open.id === "archive") {
    return "agent.archive.continue";
  }
  return dialogTitleKey(open);
}
