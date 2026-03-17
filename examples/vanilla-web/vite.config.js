import { defineConfig } from 'vite';
import path from 'path';

export default defineConfig({
  optimizeDeps: {
    exclude: ['@auser/workflow-graph-web'],
  },
  server: {
    fs: {
      // Allow serving files from the entire monorepo (needed for WASM binary in packages/web/wasm/)
      allow: [path.resolve(__dirname, '../..')],
    },
  },
});
