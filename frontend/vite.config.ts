import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

const backend = process.env["SOABOX_DEV_BACKEND"] ?? "http://localhost:8080";
const isVitest = process.env["VITEST"] !== undefined;

export default defineConfig({
  plugins: [
    // The router plugin must come before the React plugin. Tests use the committed route tree.
    ...(isVitest ? [] : [tanstackRouter({ target: "react", autoCodeSplitting: true })]),
    react(),
    tailwindcss(),
  ],
  server: {
    proxy: {
      "/api": { target: backend, changeOrigin: false },
      "/v1": { target: backend, changeOrigin: false },
      "/auth": { target: backend, changeOrigin: false },
    },
  },
  build: {
    outDir: "dist",
    sourcemap: false,
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
    testTimeout: 15000,
    coverage: {
      provider: "v8",
      include: ["src/**/*.{ts,tsx}"],
      exclude: ["src/**/*.test.{ts,tsx}", "src/test/**", "src/routeTree.gen.ts", "src/main.tsx", "src/vite-env.d.ts"],
      reporter: ["text-summary", "text", "html"],
      thresholds: {
        lines: 80,
      },
    },
  },
});
