import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import path from "node:path";

// Tauri 的开发服务器约定
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [vue()],

  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },

  // Vite 把 monaco-editor 的 worker 当作资源优化
  optimizeDeps: {
    include: ["monaco-editor/esm/vs/editor/editor.api"],
  },

  // Tauri 要求的配置
  clearScreen: false,
  server: {
    // Tauri 默认端口
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 忽略 Rust 源码，避免不必要重启
      ignored: ["**/src-tauri/**"],
    },
  },
}));
