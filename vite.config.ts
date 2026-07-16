import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Porta 1420 é o padrão esperado pelo Tauri (ver tauri.conf.json > build.devUrl)
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // não observar o crate Rust
      ignored: ["**/src-tauri/**"],
    },
  },
});
