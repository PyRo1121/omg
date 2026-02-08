import { defineWorkersConfig } from '@cloudflare/vitest-pool-workers/config';

export default defineWorkersConfig({
  test: {
    globals: true,
    poolOptions: {
      workers: {
        wrangler: { configPath: './wrangler.toml' },
        miniflare: {
          // Use in-memory D1 databases for testing
          d1Databases: ['DB', 'ANALYTICS_DB'],
        },
      },
    },
  },
});
