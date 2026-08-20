"use client";

import {
  Card,
  Input,
  List,
  ListItem,
  SectionHeader,
  SegmentedControl,
  Switch,
  type InputProps,
  type ListItemProps,
  type SegmentedControlProps,
} from "@layerx/ui";
import type { ComponentProps, ReactNode } from "react";

export function SettingsSection({
  title,
  children,
}: Readonly<{ title: ReactNode; children: ReactNode }>) {
  return (
    <Card elevation="outline" className="flex flex-col gap-3">
      <SectionHeader title={title} />
      <List>{children}</List>
    </Card>
  );
}

export type SettingsRowProps = ListItemProps;

export function SettingsRow(props: SettingsRowProps) {
  return <ListItem {...props} />;
}

export function SettingsSwitch(
  props: ComponentProps<typeof Switch> & Readonly<{ label: string }>,
) {
  const { label, ...switchProps } = props;
  return <Switch {...switchProps} aria-label={label} />;
}

export function SettingsTextInput(props: InputProps) {
  return <Input {...props} />;
}

export function SettingsSegmentedControl(props: SegmentedControlProps) {
  return <SegmentedControl {...props} />;
}
