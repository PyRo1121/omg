#!/bin/bash
# Deploy Docs Analytics System to Production
# Run this script to deploy the analytics system in one command

set -e  # Exit on error

echo "🚀 Deploying Docs Analytics System"
echo "=================================="

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Step 1: Run database migration
echo -e "\n${BLUE}📦 Step 1/4: Running database migration...${NC}"
bunx wrangler d1 execute omg-licensing --remote --file=./migrations/008-docs-analytics.sql

# Verify tables
echo -e "${GREEN}✓ Verifying tables...${NC}"
bunx wrangler d1 execute omg-licensing --remote --command="SELECT COUNT(*) as table_count FROM sqlite_master WHERE type='table' AND name LIKE 'docs_analytics%';" --json

# Step 2: Deploy worker
echo -e "\n${BLUE}🔧 Step 2/4: Deploying Cloudflare Worker...${NC}"
bunx wrangler deploy

# Step 3: Test analytics endpoint
echo -e "\n${BLUE}🧪 Step 3/4: Testing analytics endpoint...${NC}"
TEST_TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
TEST_RESPONSE=$(curl -s -X POST https://api.pyro1121.com/api/docs/analytics \
  -H "Content-Type: application/json" \
  -d '{
    "events": [{
      "event_type": "pageview",
      "event_name": "page_view",
      "properties": {
        "url": "/test-deployment",
        "referrer": "deployment-script",
        "viewport": "1920x1080"
      },
      "timestamp": "'${TEST_TIMESTAMP}'",
      "session_id": "deployment_test_'$(date +%s)'"
    }]
  }')

if echo "$TEST_RESPONSE" | grep -q '"success":true'; then
  echo -e "${GREEN}✓ Analytics endpoint working!${NC}"
  echo "$TEST_RESPONSE" | jq .
else
  echo -e "${YELLOW}⚠ Analytics endpoint returned unexpected response:${NC}"
  echo "$TEST_RESPONSE"
fi

# Step 4: Deploy docs site
echo -e "\n${BLUE}📚 Step 4/4: Deploying docs site...${NC}"
cd ../../docs-site

# Build docs
echo "Building docs site..."
npm run build

# Deploy to Cloudflare Pages
echo "Deploying to Cloudflare Pages..."
npx wrangler pages deploy build --project-name=omg-docs

cd ../site/workers

# Final verification
echo -e "\n${GREEN}✅ Deployment Complete!${NC}"
echo ""
echo "📊 Analytics Dashboard: https://api.pyro1121.com/api/docs/analytics/dashboard"
echo "🌐 Docs Site: https://omg-docs.pages.dev"
echo ""
echo "Next steps:"
echo "1. Visit the docs site and interact with some pages"
echo "2. Wait 5-10 seconds for batch flush"
echo "3. Check dashboard: curl https://api.pyro1121.com/api/docs/analytics/dashboard | jq"
echo "4. View real-time logs: bunx wrangler tail"
echo ""
echo "For more details, see: DOCS_ANALYTICS_SETUP.md"
