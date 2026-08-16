import { defineConfig } from "tsup";

export default defineConfig({
  entry: ["src/index.ts"],
  format: ["esm", "cjs"],
  dts: true,
  sourcemap: true,
  clean: true,
  external: ["react", "react-dom", "tailwindcss"],
  banner: { js: '"use client";' },
  esbuildOptions(options) {
    options.jsx = "automatic";
  },
});
