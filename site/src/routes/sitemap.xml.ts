import type { APIEvent } from '@solidjs/start/server';

// Site configuration
const SITE_URL = 'https://pyro1121.com';

// Static pages with their priorities and change frequencies
interface PageEntry {
  path: string;
  priority: number;
  changefreq: 'always' | 'hourly' | 'daily' | 'weekly' | 'monthly' | 'yearly' | 'never';
  lastmod?: string;
}

const STATIC_PAGES: PageEntry[] = [
  { path: '/', priority: 1.0, changefreq: 'weekly' },
  { path: '/docs', priority: 0.9, changefreq: 'weekly' },
  { path: '/login', priority: 0.3, changefreq: 'monthly' },
  { path: '/signup', priority: 0.5, changefreq: 'monthly' },
  { path: '/dashboard', priority: 0.4, changefreq: 'monthly' },
];

// Documentation pages - these would ideally be generated dynamically
// but we'll define common ones here for now
const DOC_PAGES: PageEntry[] = [
  { path: '/docs/getting-started', priority: 0.8, changefreq: 'weekly' },
  { path: '/docs/installation', priority: 0.8, changefreq: 'weekly' },
  { path: '/docs/commands', priority: 0.8, changefreq: 'weekly' },
  { path: '/docs/configuration', priority: 0.7, changefreq: 'weekly' },
  { path: '/docs/runtimes', priority: 0.7, changefreq: 'weekly' },
  { path: '/docs/runtimes/nodejs', priority: 0.6, changefreq: 'monthly' },
  { path: '/docs/runtimes/python', priority: 0.6, changefreq: 'monthly' },
  { path: '/docs/runtimes/go', priority: 0.6, changefreq: 'monthly' },
  { path: '/docs/runtimes/rust', priority: 0.6, changefreq: 'monthly' },
  { path: '/docs/runtimes/ruby', priority: 0.6, changefreq: 'monthly' },
  { path: '/docs/runtimes/java', priority: 0.6, changefreq: 'monthly' },
  { path: '/docs/runtimes/bun', priority: 0.6, changefreq: 'monthly' },
  { path: '/docs/package-managers', priority: 0.7, changefreq: 'weekly' },
  { path: '/docs/package-managers/arch', priority: 0.6, changefreq: 'monthly' },
  { path: '/docs/package-managers/debian', priority: 0.6, changefreq: 'monthly' },
  { path: '/docs/package-managers/fedora', priority: 0.6, changefreq: 'monthly' },
  { path: '/docs/aur', priority: 0.7, changefreq: 'weekly' },
  { path: '/docs/fleet', priority: 0.7, changefreq: 'weekly' },
  { path: '/docs/daemon', priority: 0.6, changefreq: 'monthly' },
  { path: '/docs/security', priority: 0.6, changefreq: 'monthly' },
  { path: '/docs/faq', priority: 0.5, changefreq: 'monthly' },
  { path: '/docs/changelog', priority: 0.5, changefreq: 'weekly' },
  { path: '/docs/contributing', priority: 0.4, changefreq: 'monthly' },
  { path: '/docs/license', priority: 0.3, changefreq: 'yearly' },
];

/**
 * Generate XML for a single URL entry
 */
function generateUrlEntry(page: PageEntry, baseUrl: string): string {
  const loc = `${baseUrl}${page.path}`;
  const lastmod = page.lastmod || new Date().toISOString().split('T')[0];

  return `  <url>
    <loc>${escapeXml(loc)}</loc>
    <lastmod>${lastmod}</lastmod>
    <changefreq>${page.changefreq}</changefreq>
    <priority>${page.priority.toFixed(1)}</priority>
  </url>`;
}

/**
 * Escape special XML characters
 */
function escapeXml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;');
}

/**
 * Generate the complete sitemap XML
 */
function generateSitemap(): string {
  const allPages = [...STATIC_PAGES, ...DOC_PAGES];

  const urlEntries = allPages.map((page) => generateUrlEntry(page, SITE_URL)).join('\n');

  return `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"
        xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
        xsi:schemaLocation="http://www.sitemaps.org/schemas/sitemap/0.9
                            http://www.sitemaps.org/schemas/sitemap/0.9/sitemap.xsd">
${urlEntries}
</urlset>`;
}

/**
 * API route handler for sitemap.xml
 */
export function GET(_event: APIEvent): Response {
  const sitemap = generateSitemap();

  return new Response(sitemap, {
    status: 200,
    headers: {
      'Content-Type': 'application/xml; charset=utf-8',
      'Cache-Control': 'public, max-age=3600, s-maxage=86400',
      'X-Robots-Tag': 'noindex',
    },
  });
}
