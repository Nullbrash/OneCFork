import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// 17120 — вне диапазона временных портов Windows (1024–15000) и не пересекается
// с OnePunchBotGUI (17110): оба проекта можно запускать одновременно.
const DEV_PORT = 17120;

export default defineConfig({
  plugins: [react()],
  // Не прятать ошибки Rust за очисткой экрана Vite.
  clearScreen: false,
  server: {
    port: DEV_PORT,
    // Tauri ждёт интерфейс строго на этом порту — занят, значит ошибка, а не другой порт.
    strictPort: true,
    host: "127.0.0.1",
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    target: "es2022",
  },
  test: {
    include: ["src/**/*.test.ts"],
  },
});
