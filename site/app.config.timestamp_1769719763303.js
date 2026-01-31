// app.config.ts
import { defineConfig } from "@solidjs/start/config";
var app_config_default = defineConfig({
  server: {
    preset: "cloudflare-pages",
    rollupConfig: {
      external: ["node:async_hooks"]
    },
    prerender: {
      routes: ["/", "/docs", "/login", "/signup"],
      crawlLinks: true,
      ignore: ["/dashboard", "/api/*"]
    }
  },
  vite: {
    server: {
      port: 3e3
    },
    build: {
      target: "esnext",
      minify: "esbuild"
    },
    css: {
      postcss: "./postcss.config.js"
    }
  }
});
export {
  app_config_default as default
};
