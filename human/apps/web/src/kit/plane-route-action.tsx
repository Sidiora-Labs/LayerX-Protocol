"use client";

import { useRouter } from "next/navigation";
import type { ReactNode } from "react";

import { KitButton } from "./control";

export function PlaneRouteAction({
  destination,
  children,
}: Readonly<{ destination: "/app" | "/explorer"; children: ReactNode }>) {
  const router = useRouter();
  return <KitButton onClick={() => { router.push(destination); }}>{children}</KitButton>;
}
