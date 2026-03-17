import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      // Resolve to source — Vite compiles on the fly
      '@auser/workflow-graph-react': path.resolve(__dirname, '../../packages/react/src/index.tsx'),
      '@auser/workflow-graph-web': path.resolve(__dirname, '../../packages/web/src/index.ts'),
    },
  },
  server: {
    fs: {
      allow: [path.resolve(__dirname, '../..')],
    },
  },
  // Serve WASM files as static assets
  publicDir: path.resolve(__dirname, '../../packages/web/wasm'),
});
