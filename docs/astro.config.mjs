import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://auser.github.io',
  base: '/workflow-graph',
  integrations: [
    starlight({
      title: 'workflow-graph',
      description:
        'A GitHub Actions-style workflow DAG visualizer and job execution engine',
      social: {
        github: 'https://github.com/auser/workflow-graph',
      },
      sidebar: [
        {
          label: 'Getting Started',
          items: [
            { label: 'Installation', slug: 'getting-started/installation' },
            { label: 'Quick Start', slug: 'getting-started/quick-start' },
          ],
        },
        {
          label: 'Architecture',
          items: [
            { label: 'Overview', slug: 'architecture/overview' },
            {
              label: 'Deployment Modes',
              slug: 'architecture/deployment-modes',
            },
          ],
        },
        {
          label: 'API Reference',
          items: [
            { label: 'REST API', slug: 'api/rest-api' },
            { label: 'WASM API', slug: 'api/wasm-api' },
          ],
        },
        {
          label: 'Workers',
          items: [
            { label: 'Overview', slug: 'workers/overview' },
            { label: 'Worker SDK', slug: 'workers/sdk' },
            { label: 'Custom Workers', slug: 'workers/custom-workers' },
            {
              label: 'Labels & Outputs',
              slug: 'workers/labels-and-outputs',
            },
          ],
        },
        {
          label: 'Guides',
          items: [
            {
              label: 'Workflow Definitions',
              slug: 'guides/workflow-definitions',
            },
            { label: 'Creating Workers', slug: 'guides/creating-workers' },
            { label: 'Embedding', slug: 'guides/embedding' },
            { label: 'Custom Queue Backend', slug: 'guides/custom-queue' },
            { label: 'Postgres / pg-boss', slug: 'guides/postgres-backend' },
            { label: 'Redis Backend', slug: 'guides/redis-backend' },
          ],
        },
      ],
    }),
  ],
});
