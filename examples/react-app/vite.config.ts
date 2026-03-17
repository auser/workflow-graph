import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      // Point directly at source so Vite compiles it — no pre-build needed
      '@auser/workflow-graph-react': path.resolve(__dirname, '../../packages/react/src/index.tsx'),
      '@auser/workflow-graph-web': path.resolve(__dirname, '../../packages/web/src/index.ts'),
    },
  },
  server: {
    fs: {
      allow: [path.resolve(__dirname, '../..')],
    },
  },
});
