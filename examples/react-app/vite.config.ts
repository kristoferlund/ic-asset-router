import { defineConfig } from "vite";
import react from "@vitejs/plugin-react-swc";
import dotenv from "dotenv";
import environment from "vite-plugin-environment";
import { fileURLToPath } from "url";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import { icpBindgen } from "@icp-sdk/bindgen/plugins/vite";

dotenv.config({ path: ".env" });

export default defineConfig({
  build: {
    outDir: "dist",
  },
  optimizeDeps: {
    esbuildOptions: {
      define: {
        global: "globalThis",
      },
    },
  },
  plugins: [
    tanstackRouter({ target: "react", autoCodeSplitting: true }),
    react(),
    environment("all", { prefix: "CANISTER_" }),
    environment("all", { prefix: "DFX_" }),
    icpBindgen({
      didFile: "./server/server.did",
      outDir: "./src",
    }),
  ],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src/", import.meta.url)),
    },
  },
});
