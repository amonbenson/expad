import { fileURLToPath, URL } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";
import { defineConfig, loadEnv } from "vite";
import { compression } from "vite-plugin-compression2";
import { viteSingleFile } from "vite-plugin-singlefile";

// Shares the repository root .env with build.rs
const envDir = fileURLToPath(new URL("..", import.meta.url));

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const { EXPAD_DEVICE_ADDRESS = "192.168.4.1" } = loadEnv(mode, envDir, "EXPAD_");

  return {
    envDir,
    build: {
      // Set by build.rs to build straight into cargo's OUT_DIR
      outDir: process.env.EXPAD_WEB_OUT_DIR,
      emptyOutDir: true,
    },
    // The firmware embeds the web interface as a single gzipped index.html
    plugins: [
      vue(),
      tailwindcss(),
      viteSingleFile(),
      compression({ algorithms: ["gzip"], deleteOriginalAssets: true }),
    ],
    resolve: {
      alias: {
        "@": fileURLToPath(new URL("./src", import.meta.url)),
      },
    },
    server: {
      proxy: {
        "/ws": { target: `ws://${EXPAD_DEVICE_ADDRESS}`, ws: true },
      },
    },
  };
});
