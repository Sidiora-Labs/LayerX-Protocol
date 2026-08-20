"use client";

import {
  AmountText,
  Badge,
  Button,
  Card,
  DetailDisclosure,
  Input,
  cn,
  type AmountTextProps,
  type PlatformSetting,
} from "@layerx/ui";
import { useState, type ReactNode } from "react";

import { copyEntry } from "../../copy/catalog.ts";
import {
  directionWord,
  feeMathTotal,
  statusPresentation,
  type MoneyDirection,
  type ProtocolAmount,
  type StatusKey,
} from "./model";

export type SignedWordedAmountProps = Omit<AmountTextProps, "colorMode" | "value"> & Readonly<{
  value: ProtocolAmount;
  readonly direction?: MoneyDirection;
}>;

export function SignedWordedAmount({ value, direction, className, ...props }: SignedWordedAmountProps) {
  const resolvedDirection = direction ?? (value > 0 ? "inbound" : value < 0 ? "outbound" : "other");
  return (
    <span className={cn("inline-flex items-baseline gap-2", className)}>
      <AmountText {...props} value={value} colorMode="signed" />
      <span className="text-sm font-medium text-foreground-secondary">
        {directionWord(resolvedDirection)}
      </span>
    </span>
  );
}

export function LabelValue({
  label,
  value,
  className,
}: Readonly<{ label: ReactNode; value: ReactNode; className?: string }>) {
  return (
    <span className={cn("inline-flex flex-col gap-1", className)}>
      <span className="text-sm text-muted-foreground">{label}</span>
      <span className="font-semibold tabular-nums text-foreground">{value}</span>
    </span>
  );
}

export function StatusPill({ status, className }: Readonly<{ status: StatusKey; className?: string }>) {
  const presentation = statusPresentation(status);
  return (
    <Badge variant={presentation.tone} className={className}>
      {presentation.label}
    </Badge>
  );
}

export function CopyableIdentifier({
  label,
  value,
  onCopied,
  className,
}: Readonly<{ label: string; value: string; onCopied?: (value: string) => void; className?: string }>) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      onCopied?.(value);
    } catch {
      setCopied(false);
    }
  };

  return (
    <label className={cn("flex flex-col gap-2 text-sm font-semibold text-foreground", className)}>
      {label}
      <Input
        readOnly
        value={value}
        className="tabular-nums"
        trailing={
          <Button type="button" variant="link" size="sm" onClick={() => void copy()}>
            {copyEntry(copied ? "action.copied" : "action.copy").message}
          </Button>
        }
      />
    </label>
  );
}

export interface FeeMathStep {
  readonly label: string;
  readonly amount: ProtocolAmount;
  readonly explanation?: string;
}

export interface FeeMathDisclosureProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly steps: readonly [FeeMathStep, ...FeeMathStep[]];
  readonly title?: string;
  readonly totalLabel?: string;
  readonly currency?: string;
  readonly symbol?: string;
  readonly decimals?: number;
  readonly platform: PlatformSetting;
  readonly portalContainer?: HTMLElement | null;
}

export function FeeMathDisclosure({
  open,
  onOpenChange,
  steps,
  title = copyEntry("fee.disclosure.title").message,
  totalLabel = copyEntry("fee.disclosure.total").message,
  currency,
  symbol,
  decimals,
  platform,
  portalContainer,
}: FeeMathDisclosureProps) {
  const amountProps = {
    ...(currency === undefined ? {} : { currency }),
    ...(symbol === undefined ? {} : { symbol }),
    ...(decimals === undefined ? {} : { decimals }),
  };
  const total = feeMathTotal(steps);

  return (
    <DetailDisclosure
      open={open}
      onOpenChange={onOpenChange}
      title={title}
      summary={title}
      desktopVariant="inline"
      platform={platform}
      {...(portalContainer === undefined ? {} : { portalContainer })}
    >
      <ol className="flex flex-col gap-3">
        {steps.map((step, index) => (
          <li key={`${String(index)}-${step.label}`} className="flex items-start gap-3">
            <span className="inline-flex size-7 shrink-0 items-center justify-center rounded-full bg-surface-sunken text-sm font-bold tabular-nums text-foreground">
              {index + 1}
            </span>
            <Card elevation="outline" padding="sm" className="flex min-w-0 flex-1 items-start justify-between gap-4">
              <span className="flex flex-col gap-1">
                <span className="font-semibold text-foreground">{step.label}</span>
                {step.explanation === undefined ? null : (
                  <span className="text-sm text-muted-foreground">{step.explanation}</span>
                )}
              </span>
              <AmountText {...amountProps} value={step.amount} colorMode="neutral" />
            </Card>
          </li>
        ))}
      </ol>
      <div className="mt-4 flex items-center justify-between border-t border-border pt-4">
        <span className="font-semibold text-foreground">{totalLabel}</span>
        <AmountText {...amountProps} value={total} colorMode="neutral" />
      </div>
    </DetailDisclosure>
  );
}
