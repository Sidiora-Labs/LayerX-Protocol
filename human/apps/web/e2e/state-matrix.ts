import { copyEntry } from "../copy/catalog.ts";

export const REQUIRED_SCREEN_STATES = [
  "loading",
  "empty",
  "error",
  "offline",
  "degraded",
  "still-checking",
] as const;

export type ScreenState = (typeof REQUIRED_SCREEN_STATES)[number];
export type HumanShell = "mobile" | "desktop";

export interface StatePresentation {
  readonly titleKey: string;
  readonly actionKeys: readonly string[];
  readonly duplicateActionsLocked: boolean;
}

export interface ScreenStateEntry {
  readonly id: string;
  readonly route: string;
  readonly shells: readonly HumanShell[];
  readonly states: Readonly<Record<ScreenState, () => StatePresentation>>;
}

const presentation = (
  titleKey: string,
  actionKeys: readonly string[] = [],
  duplicateActionsLocked = false,
): (() => StatePresentation) =>
  () => Object.freeze({ titleKey, actionKeys: Object.freeze(actionKeys), duplicateActionsLocked });

const currentScreens: readonly ScreenStateEntry[] = Object.freeze([
  Object.freeze({
    id: "root",
    route: "/",
    shells: Object.freeze(["mobile", "desktop"] as const),
    states: Object.freeze({
      loading: presentation("state.loading"),
      empty: presentation("state.empty"),
      error: presentation("state.error", ["action.retry", "action.reload", "action.report"]),
      offline: presentation("state.offline", ["action.retry", "action.report"]),
      degraded: presentation("state.degraded", ["action.reload", "action.report"]),
      "still-checking": presentation("status.still_checking", [], true),
    }),
  }),
]);

export class StateMatrixRegistry {
  readonly #entries: ReadonlyMap<string, ScreenStateEntry>;

  constructor(entries: readonly ScreenStateEntry[]) {
    this.#entries = new Map(entries.map((entry) => [entry.id, entry]));
    if (this.#entries.size !== entries.length) {
      throw new Error("state-matrix screen identifiers must be unique");
    }
  }

  enumerate(): readonly ScreenStateEntry[] {
    return Object.freeze([...this.#entries.values()]);
  }

  assertComplete(routes: readonly string[]): void {
    const registeredRoutes = new Set<string>();
    for (const entry of this.#entries.values()) {
      registeredRoutes.add(entry.route);
      if (entry.shells.length !== 2 || !entry.shells.includes("mobile") || !entry.shells.includes("desktop")) {
        throw new Error(`${entry.id} must declare both shells`);
      }
      for (const state of REQUIRED_SCREEN_STATES) {
        const renderer = entry.states[state];
        if (typeof renderer !== "function") {
          throw new Error(`${entry.id} is missing ${state}`);
        }
        const rendered = renderer();
        copyEntry(rendered.titleKey);
        for (const actionKey of rendered.actionKeys) {
          copyEntry(actionKey);
        }
        if (state === "still-checking" && !rendered.duplicateActionsLocked) {
          throw new Error(`${entry.id} must lock duplicate actions while still checking`);
        }
      }
    }
    for (const route of routes) {
      if (!registeredRoutes.has(route)) {
        throw new Error(`${route} has no state-matrix registration`);
      }
    }
  }
}

export function human_state_matrix_registry(): StateMatrixRegistry {
  return new StateMatrixRegistry(currentScreens);
}
