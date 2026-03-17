import { defineConfig } from 'vite';

export default defineConfig({
  optimizeDeps: {
    exclude: ['@auser/workflow-graph-web'],
  },
});
