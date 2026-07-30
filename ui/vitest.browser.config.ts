import preact from "@preact/preset-vite";
import { playwright } from "@vitest/browser-playwright";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [preact()],
  test: {
    include: ["src/**/*.browser.test.tsx"],
    browser: {
      enabled: true,
      headless: true,
      screenshotFailures: false,
      provider: playwright({
        launchOptions: process.env.CI ? {} : { channel: "chrome" },
      }),
      instances: [{ browser: "chromium" }],
      viewport: {
        width: 420,
        height: 680,
      },
    },
  },
});
