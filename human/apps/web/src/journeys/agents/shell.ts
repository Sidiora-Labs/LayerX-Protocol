"use client";

import { useEffect, useState } from "react";

import type { AgentsShell } from "./model.ts";

export function useAgentsShell(initial: AgentsShell): AgentsShell {
  const [shell, setShell] = useState<AgentsShell>(initial);
  useEffect(() => {
    const resolved = document.querySelector<HTMLElement>("[data-shell]")?.dataset.shell;
    if (resolved === "mobile" || resolved === "desktop") {
      setShell(resolved);
    }
  }, []);
  return shell;
}
