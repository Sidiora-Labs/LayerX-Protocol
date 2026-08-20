export const SHELL_PROFILES = Object.freeze({
  mobile: Object.freeze({
    projectName: "mobile-shell",
    viewport: Object.freeze({ width: 390, height: 844 }),
    hasTouch: true,
    isMobile: true,
  }),
  desktop: Object.freeze({
    projectName: "desktop-shell",
    viewport: Object.freeze({ width: 1440, height: 960 }),
    hasTouch: false,
    isMobile: false,
  }),
});

export interface HumanTestHarness {
  readonly baseUrl: string;
  readonly realStack: true;
  readonly localProduction: boolean;
  readonly traceDirectory: string;
  readonly visualBaselineReviewed: boolean;
}

type EnvironmentValues = Readonly<Record<string, string | undefined>>;

function required(values: EnvironmentValues, name: string): string {
  const value = values[name]?.trim();
  if (value === undefined || value.length === 0) {
    throw new Error(`${name} is required for the real-stack browser harness`);
  }
  return value;
}

export function human_test_harness(values: EnvironmentValues): HumanTestHarness {
  if (required(values, "HUMAN_E2E_REAL_STACK") !== "1") {
    throw new Error("HUMAN_E2E_REAL_STACK=1 is required; substitutes are not accepted");
  }
  const candidate = new URL(required(values, "HUMAN_E2E_BASE_URL"));
  if (candidate.protocol !== "https:" && candidate.protocol !== "http:") {
    throw new Error("HUMAN_E2E_BASE_URL must use HTTP or HTTPS");
  }
  if (candidate.username.length > 0 || candidate.password.length > 0) {
    throw new Error("HUMAN_E2E_BASE_URL must not contain credentials");
  }
  const localProduction = values.HUMAN_E2E_LOCAL_PRODUCTION === "1";
  if (localProduction && candidate.origin !== "http://127.0.0.1:3105") {
    throw new Error("The local production harness is pinned to http://127.0.0.1:3105");
  }
  return Object.freeze({
    baseUrl: candidate.toString(),
    realStack: true,
    localProduction,
    traceDirectory: values.HUMAN_E2E_TRACE_DIRECTORY?.trim() || "test-results/traces",
    visualBaselineReviewed: values.HUMAN_VISUAL_BASELINE_REVIEWED === "1",
  });
}
