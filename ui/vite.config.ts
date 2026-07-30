import preact from "@preact/preset-vite";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [preact()],
  clearScreen: false,
  test: {
    include: ["src/**/*.test.{ts,tsx}"],
    exclude: ["src/**/*.browser.test.tsx"],
  },
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
  },
});
