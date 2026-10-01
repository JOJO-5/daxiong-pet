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
    // 不让 vite 自动清空 outDir：本机有 fs.rmSync 安全钩子会拦截批量删除，
    // 导致构建中断。需要干净产物时先手动删除 dist 目录即可。
    emptyOutDir: false,
  },
});
