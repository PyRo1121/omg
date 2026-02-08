import type { APIEvent } from '@solidjs/start/server';

// Site configuration
const SITE_URL = 'https://pyro1121.com';

/**
 * Generate robots.txt content
 *
 * This allows all crawlers access to the site while:
 * - Disallowing admin and API routes
 * - Referencing the sitemap for discovery
 * - Setting a reasonable crawl delay
 */
function generateRobotsTxt(): string {
  return `# OMG Package Manager - robots.txt
# https://pyro1121.com

# Allow all crawlers
User-agent: *

# Disallow admin and private routes
Disallow: /admin
Disallow: /admin/
Disallow: /api/
Disallow: /dashboard
Disallow: /dashboard/

# Disallow authentication routes
Disallow: /login
Disallow: /signup

# Disallow utility routes
Disallow: /_
Disallow: /*.json$

# Allow important static assets
Allow: /logo-globe.png
Allow: /og/

# Crawl delay (seconds) - be nice to our servers
Crawl-delay: 1

# Sitemap location
Sitemap: ${SITE_URL}/sitemap.xml

# Specific rules for aggressive bots
User-agent: GPTBot
Disallow: /

User-agent: ChatGPT-User
Disallow: /

User-agent: CCBot
Disallow: /

User-agent: anthropic-ai
Disallow: /

User-agent: Google-Extended
Disallow: /

User-agent: Bytespider
Disallow: /

User-agent: ClaudeBot
Disallow: /
`;
}

/**
 * API route handler for robots.txt
 */
export function GET(_event: APIEvent): Response {
  const robotsTxt = generateRobotsTxt();

  return new Response(robotsTxt, {
    status: 200,
    headers: {
      'Content-Type': 'text/plain; charset=utf-8',
      'Cache-Control': 'public, max-age=86400, s-maxage=604800',
    },
  });
}
