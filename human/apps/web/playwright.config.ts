import { defineConfig } from "@playwright/test";

import { SHELL_PROFILES, human_test_harness } from "./e2e/harness.ts";

const harness = human_test_harness(process.env);

export default defineConfig({
  testDir: "./e2e/browser",
  testMatch: ["**/*.spec.ts"],
  outputDir: harness.traceDirectory,
  fullyParallel: false,
  forbidOnly: true,
  retries: process.env.CI === "true" ? 2 : 0,
  reporter: [["line"], ["html", { open: "never", outputFolder: "test-results/report" }]],
  snapshotPathTemplate: "{testDir}/__screenshots__/{projectName}/{arg}{ext}",
  use: {
    baseURL: harness.baseUrl,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  projects: [
    ...Object.values(SHELL_PROFILES).map((profile) => ({
      name: profile.projectName,
      use: {
        viewport: profile.viewport,
        hasTouch: profile.hasTouch,
        isMobile: profile.isMobile,
      },
    })),
    {
      name: "touch-tablet-shell",
      testMatch: "**/a11y.spec.ts",
      use: {
        viewport: { width: 1024, height: 1366 },
        hasTouch: true,
        isMobile: true,
      },
    },
  ],
});
