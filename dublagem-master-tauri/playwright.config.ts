import { defineConfig, devices } from "@playwright/test";

const e2ePort = process.env.PLAYWRIGHT_PORT ?? "1430";
const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? `http://127.0.0.1:${e2ePort}`;
const skipWebServer = process.env.PLAYWRIGHT_SKIP_WEB_SERVER === "1";

export default defineConfig({
  testDir: "./tests/e2e",
  timeout: 30_000,
  fullyParallel: false,
  use: {
    baseURL,
    trace: "retain-on-failure"
  },
  webServer: skipWebServer
    ? undefined
    : {
        command: `npm run dev -- --host 127.0.0.1 --port ${e2ePort} --strictPort`,
        url: baseURL,
        reuseExistingServer: false,
        timeout: 120_000
      },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] }
    }
  ]
});
