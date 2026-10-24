import { defineConfig, devices } from "@playwright/test";

// The frontend tests (Playwright Test).
//
// - `unit`: pure TypeScript, no browser.
// - `page`: the built frontend (`vite preview` of dist/) in WebKit, the engine of the system
//   WebView, with the Tauri commands mocked (tests/page/tauri-mock.ts). Needs Playwright's
//   WebKit installed (`pnpm exec playwright install webkit`), as `scripts/dev.sh frontend` and
//   CI do; without it, run only the `unit` project.
export default defineConfig({
  testDir: "tests",
  forbidOnly: true,
  reporter: "list",
  projects: [
    { name: "unit", testMatch: "unit/**/*.test.ts" },
    {
      name: "page",
      testMatch: "page/**/*.spec.ts",
      use: { ...devices["Desktop Safari"], baseURL: "http://localhost:4173" },
    },
  ],
  webServer: {
    command: "pnpm run preview",
    url: "http://localhost:4173",
    reuseExistingServer: false,
  },
});
