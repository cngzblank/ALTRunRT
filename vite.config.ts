import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 9200,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 9201 }
      : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
}));
