import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

export default defineConfig({
  plugins: [solid()],
  server: {
    port: 3000,
  },
  build: {
    target: 'esnext',
    rollupOptions: {
      output: {
        manualChunks: id => {
          if (id.includes('node_modules')) {
            if (id.includes('solid-js') || id.includes('@solidjs/router')) {
              return 'solid-vendor';
            }
            if (id.includes('@tanstack')) {
              return 'tanstack';
            }
            if (id.includes('lucide-solid')) {
              return 'icons';
            }
            if (id.includes('three')) {
              return 'three';
            }
            if (id.includes('@sentry')) {
              return 'sentry';
            }
            if (id.includes('apexcharts') || id.includes('solid-apexcharts')) {
              return 'charts';
            }
          }
        },
      },
    },
  },
});
