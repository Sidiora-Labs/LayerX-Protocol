"use client";

import {
  AppShell,
  CodeEntry,
  DetailDisclosure,
  FilterBar,
  GlobalSearch,
  MoneyList,
  NotificationsArchive,
  NotificationsScreen,
  BellPopover,
  PrimaryAction,
  Wizard,
  cn,
} from "@layerx/ui";
import { useId, type ComponentProps } from "react";

import { DisabledReason, REDUCED_MOTION_CLASS, type ControlAvailability } from "./control";

export type NavigationProps = Omit<ComponentProps<typeof AppShell>, "platform">;

function Navigation({ platform, ...props }: NavigationProps & Readonly<{ platform: "mobile" | "desktop" }>) {
  return <AppShell {...props} platform={platform} />;
}

export function MobileNavigation(props: NavigationProps) {
  return <Navigation {...props} platform="mobile" />;
}

export function DesktopNavigation(props: NavigationProps) {
  return <Navigation {...props} platform="desktop" />;
}

type PrimaryActionBaseProps = Omit<
  ComponentProps<typeof PrimaryAction>,
  "platform" | "disabled" | "aria-describedby"
>;
export type KitPrimaryActionProps = PrimaryActionBaseProps & ControlAvailability;

function PrimaryActionFor({
  platform,
  disabled = false,
  disabledReason,
  className,
  ...props
}: KitPrimaryActionProps & Readonly<{ platform: "mobile" | "desktop" }>) {
  const reasonId = useId();
  return (
    <div className="flex flex-col">
      <PrimaryAction
        {...props}
        platform={platform}
        disabled={disabled}
        aria-describedby={disabled ? reasonId : undefined}
        className={cn(REDUCED_MOTION_CLASS, className)}
      />
      <DisabledReason id={reasonId}>{disabled ? disabledReason : undefined}</DisabledReason>
    </div>
  );
}

export function MobilePrimaryAction(props: KitPrimaryActionProps) {
  return <PrimaryActionFor {...props} platform="mobile" />;
}

export function DesktopPrimaryAction(props: KitPrimaryActionProps) {
  return <PrimaryActionFor {...props} platform="desktop" />;
}

export type DetailProps = Omit<ComponentProps<typeof DetailDisclosure>, "platform">;

export function MobileDetail(props: DetailProps) {
  return <DetailDisclosure {...props} platform="mobile" />;
}

export function DesktopDetail(props: DetailProps) {
  return <DetailDisclosure {...props} platform="desktop" />;
}

export type FiltersProps = Omit<ComponentProps<typeof FilterBar>, "platform">;

export function MobileFilters(props: FiltersProps) {
  return <FilterBar {...props} platform="mobile" />;
}

export function DesktopFilters(props: FiltersProps) {
  return <FilterBar {...props} platform="desktop" />;
}

export type MoneyListProps = Omit<ComponentProps<typeof MoneyList>, "platform">;

export function MobileMoneyList(props: MoneyListProps) {
  return <MoneyList {...props} platform="mobile" />;
}

export function DesktopMoneyList(props: MoneyListProps) {
  return <MoneyList {...props} platform="desktop" />;
}

export type WizardProps = Omit<ComponentProps<typeof Wizard>, "platform">;

export function MobileWizard(props: WizardProps) {
  return <Wizard {...props} platform="mobile" />;
}

export function DesktopWizard(props: WizardProps) {
  return <Wizard {...props} platform="desktop" />;
}

export type SearchProps = Omit<ComponentProps<typeof GlobalSearch>, "platform">;

export function MobileSearch(props: SearchProps) {
  return <GlobalSearch {...props} platform="mobile" />;
}

export function DesktopSearch(props: SearchProps) {
  return <GlobalSearch {...props} platform="desktop" />;
}

export type CodeEntryProps = Omit<ComponentProps<typeof CodeEntry>, "platform">;

export function MobileCodeEntry(props: CodeEntryProps) {
  return <CodeEntry {...props} platform="mobile" />;
}

export function DesktopCodeEntry(props: CodeEntryProps) {
  return <CodeEntry {...props} platform="desktop" />;
}

type MobileNotificationsPrimitiveProps = ComponentProps<typeof NotificationsScreen>;
type DesktopArchivePrimitiveProps = ComponentProps<typeof NotificationsArchive>;
type DesktopPopoverPrimitiveProps = ComponentProps<typeof BellPopover>;

export type MobileNotificationsProps = MobileNotificationsPrimitiveProps;
export type DesktopNotificationsProps =
  | (DesktopArchivePrimitiveProps & Readonly<{ view: "archive" }>)
  | (DesktopPopoverPrimitiveProps & Readonly<{ view: "popover" }>);

export function MobileNotifications(props: MobileNotificationsProps) {
  return <NotificationsScreen {...props} />;
}

export function DesktopNotifications({ view, ...props }: DesktopNotificationsProps) {
  return view === "archive" ? (
    <NotificationsArchive {...(props as DesktopArchivePrimitiveProps)} />
  ) : (
    <BellPopover {...(props as DesktopPopoverPrimitiveProps)} />
  );
}
