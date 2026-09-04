import { defineConfig } from 'vite';

export default defineConfig({
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: 'chrome110',
    // Vite 8 引擎换 Rolldown，配置项随之从 rollupOptions 改名（vite 8 迁移指南）
    rolldownOptions: {
      input: { main: 'index.html', overlay: 'overlay.html', mark: 'mark.html' },
    },
  },
});
