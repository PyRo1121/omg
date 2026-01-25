#!/bin/bash
set -e

echo "🚀 Building Frontend..."
cd site
bun run build
cd ..

echo "📚 Building Documentation..."
cd docs-site
npm install
npm run build
cd ..

echo "🔌 Deploying Backend API..."
cd site/workers
bunx wrangler deploy
cd ../..

echo "☁️ Deploying Frontend to Cloudflare Pages..."
cd site
bunx wrangler pages deploy dist --project-name omg-site
cd ..

echo "☁️ Deploying Docs to Cloudflare Pages..."
cd docs-site
bunx wrangler pages deploy build --project-name omg-docs
cd ..

echo "🌐 Deploying Router Worker..."
cd workers/router
bunx wrangler deploy
cd ../..

echo "✅ Deployment Complete!"
