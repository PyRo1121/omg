#!/bin/bash
set -e

echo "🚀 Building Frontend..."
cd site
bun run build
cd ..

echo "🗄️ Migrating Database Schema..."
cd site/workers
bunx wrangler d1 execute omg-licensing --remote --file=./schema-production.sql
echo "✓ Database migration complete"

echo "🔌 Deploying Backend API..."
bunx wrangler deploy
cd ../..

echo "☁️ Deploying Frontend to Cloudflare Pages..."
cd site
bunx wrangler pages deploy dist --project-name omg-site
cd ..

echo "🌐 Deploying Router Worker..."
cd workers/router
bunx wrangler deploy
cd ../..

echo ""
echo "📚 Building Documentation (optional)..."
set +e
cd docs-site
npm install
npm run build
if [ $? -eq 0 ]; then
  echo "☁️ Deploying Docs to Cloudflare Pages..."
  bunx wrangler pages deploy build --project-name omg-docs
  echo "✓ Docs deployed successfully"
else
  echo "⚠️  Docs build failed - skipping docs deployment"
fi
cd ..
set -e

echo ""
echo "✅ Deployment Complete!"
echo ""
echo "🔗 Endpoints:"
echo "  API: https://api.pyro1121.com"
echo "  Frontend: https://pyro1121.com"
echo "  Docs: https://pyro1121.com/docs"
