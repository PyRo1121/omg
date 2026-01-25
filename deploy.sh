#!/bin/bash
set -e

echo "🚀 Building Frontend..."
cd site
bun run build

echo "☁️ Deploying to Cloudflare Pages..."
bunx wrangler pages deploy dist --project-name omg-site

echo "🌐 Deploying Router Worker..."
cd ../workers/router
bunx wrangler deploy

echo "✅ Deployment Complete!"
