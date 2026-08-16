"use client";

import * as React from "react";
import * as Dialog from "@radix-ui/react-dialog";
import { cn } from "../lib/utils";

export interface SheetProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  children: React.ReactNode;
  /** Portal target — pass a phone-frame ref in docs to contain the sheet. */
  portalContainer?: HTMLElement | null;
}

/**
 * Bottom sheet — the mobile overlay of the design set: drag handle,
 * rounded top corners, slides up over a dimmed page.
 * Esc, overlay tap, and the close affordance all dismiss (focus-trapped).
 */
export function Sheet({ open, onOpenChange, children, portalContainer }: SheetProps) {
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal container={portalContainer ?? undefined}>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-black/40 data-[state=open]:animate-fade-in data-[state=closed]:animate-fade-out" />
        <Dialog.Content
          className={cn(
            "fixed inset-x-0 bottom-0 z-50 mx-auto flex max-h-[92dvh] w-full max-w-lg flex-col",
            "rounded-t-sheet bg-surface shadow-overlay outline-none",
            "data-[state=open]:animate-sheet-up data-[state=closed]:animate-sheet-down",
          )}
        >
          {children}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

/** Top grab handle + optional centered title row. */
export function SheetHeader({
  title,
  className,
  children,
}: {
  title?: React.ReactNode;
  className?: string;
  children?: React.ReactNode;
}) {
  return (
    <div className={cn("flex flex-col items-stretch", className)}>
      <div className="flex justify-center pt-2.5 pb-1" aria-hidden>
        <span className="h-1 w-10 rounded-full bg-border-strong" />
      </div>
      {(title || children) && (
        <div className="border-b border-border px-5 pt-2 pb-4">
          {title && (
            <Dialog.Title className="text-lg font-bold text-foreground">{title}</Dialog.Title>
          )}
          {children}
        </div>
      )}
    </div>
  );
}

export function SheetDescription({
  className,
  ...props
}: React.HTMLAttributes<HTMLParagraphElement>) {
  return (
    <Dialog.Description asChild>
      <p
        className={cn("text-[15px] leading-relaxed text-foreground-secondary", className)}
        {...props}
      />
    </Dialog.Description>
  );
}

export function SheetBody({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("lx-scroll flex-1 overflow-y-auto px-5 py-4", className)} {...props} />;
}

/**
 * Stacked/paired CTAs pinned to the sheet bottom.
 * Two children render side-by-side (Clear | Apply), one renders full-width.
 */
export function SheetFooter({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn(
        "grid auto-cols-fr grid-flow-col gap-3 border-t border-border/0 px-5 pt-2 pb-6",
        className,
      )}
      {...props}
    />
  );
}
