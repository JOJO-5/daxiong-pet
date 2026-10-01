import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// 桌宠窗口尺寸固定，前端只做精灵渲染，不需要任何开发服务器特性。
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "chrome105",
    minify: "esbuild",
    sourcemap: false,
    emptyOutDir: true,
  },
});
