/**
 * ═══════════════════════════════════════════════════════════════════════════
 * DOCS PROXY HANDLER - World-Class Reverse Proxy
 * Proxies pyro1121.com/docs/* → omg-docs.pages.dev
 * Handles asset rewriting, caching, compression, and performance optimization
 * ═══════════════════════════════════════════════════════════════════════════
 */

import { Env } from '../api';

const DOCS_ORIGIN = 'https://omg-docs.pages.dev';
const CACHE_VERSION = 'v1';

/**
 * Main docs proxy handler
 * Proxies all /docs/* requests to omg-docs.pages.dev with intelligent caching
 */
export async function handleDocsProxy(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
  try {
    const url = new URL(request.url);

    // Strip /docs prefix and preserve the rest of the path
    const docsPath = url.pathname.replace(/^\/docs/, '') || '/';
    const targetUrl = `${DOCS_ORIGIN}${docsPath}${url.search}`;

    // Check cache first (for GET requests only)
    if (request.method === 'GET') {
      const cache = caches.default;
      const cacheKey = new Request(targetUrl, request);
      const cachedResponse = await cache.match(cacheKey);

      if (cachedResponse) {
        // Clone and add cache hit header
        const response = new Response(cachedResponse.body, cachedResponse);
        response.headers.set('X-Cache', 'HIT');
        response.headers.set('X-Proxy', 'Cloudflare-Worker');
        return response;
      }
    }

    // Fetch from origin
    const originRequest = new Request(targetUrl, {
      method: request.method,
      headers: prepareOriginHeaders(request.headers, url.hostname),
      body: request.method !== 'GET' && request.method !== 'HEAD' ? request.body : undefined,
      redirect: 'manual', // Handle redirects ourselves
    });

    let response = await fetch(originRequest);

    // Handle redirects
    if (response.status >= 301 && response.status <= 308) {
      const location = response.headers.get('Location');
      if (location) {
        const redirectUrl = rewriteUrl(location, url.hostname);
        return Response.redirect(redirectUrl, response.status);
      }
    }

    // Handle errors
    if (!response.ok && response.status !== 304) {
      console.error(`Docs proxy error: ${response.status} for ${targetUrl}`);

      // Return cached version on origin error if available
      if (request.method === 'GET') {
        const cache = caches.default;
        const cacheKey = new Request(targetUrl, request);
        const staleResponse = await cache.match(cacheKey);
        if (staleResponse) {
          const fallback = new Response(staleResponse.body, staleResponse);
          fallback.headers.set('X-Cache', 'STALE-ON-ERROR');
          fallback.headers.set('X-Proxy', 'Cloudflare-Worker');
          return fallback;
        }
      }
    }

    // Clone response for processing
    const contentType = response.headers.get('Content-Type') || '';
    const isHTML = contentType.includes('text/html');
    const isCSS = contentType.includes('text/css');
    const isJS = contentType.includes('javascript') || contentType.includes('ecmascript');

    // Rewrite HTML/CSS/JS to fix asset paths
    let body = response.body;
    if ((isHTML || isCSS || isJS) && response.body) {
      const text = await response.text();
      const rewritten = rewriteContent(text, isHTML, isCSS, isJS, url.hostname);
      body = new ReadableStream({
        start(controller) {
          controller.enqueue(new TextEncoder().encode(rewritten));
          controller.close();
        }
      });

      // Update response
      response = new Response(body, response);
    }

    // Add custom headers
    const finalResponse = new Response(response.body, response);
    finalResponse.headers.set('X-Cache', 'MISS');
    finalResponse.headers.set('X-Proxy', 'Cloudflare-Worker');
    finalResponse.headers.set('X-Docs-Origin', DOCS_ORIGIN);

    // Add security headers
    finalResponse.headers.set('X-Content-Type-Options', 'nosniff');
    finalResponse.headers.set('X-Frame-Options', 'SAMEORIGIN');
    finalResponse.headers.set('Referrer-Policy', 'strict-origin-when-cross-origin');

    // Cache static assets aggressively
    if (request.method === 'GET' && shouldCache(url.pathname, contentType)) {
      const cacheTtl = getCacheTtl(url.pathname, contentType);
      finalResponse.headers.set('Cache-Control', `public, max-age=${cacheTtl}, s-maxage=${cacheTtl}`);

      // Store in cache
      const cache = caches.default;
      const cacheKey = new Request(targetUrl, request);
      ctx.waitUntil(cache.put(cacheKey, finalResponse.clone()));
    }

    return finalResponse;
  } catch (error) {
    console.error('Docs proxy handler error:', error);
    return new Response('Docs temporarily unavailable', {
      status: 503,
      headers: {
        'Content-Type': 'text/plain',
        'X-Proxy-Error': 'true',
        'Retry-After': '60',
      }
    });
  }
}

/**
 * Prepare headers for origin request
 * Strips hop-by-hop headers and adds proper Host header
 */
function prepareOriginHeaders(headers: Headers, originalHost: string): Headers {
  const newHeaders = new Headers();

  // Copy all headers except hop-by-hop headers
  const hopByHopHeaders = [
    'connection',
    'keep-alive',
    'proxy-authenticate',
    'proxy-authorization',
    'te',
    'trailers',
    'transfer-encoding',
    'upgrade',
    'host', // Will set this manually
  ];

  for (const [key, value] of headers.entries()) {
    if (!hopByHopHeaders.includes(key.toLowerCase())) {
      newHeaders.set(key, value);
    }
  }

  // Set proper Host header for origin
  const originHost = new URL(DOCS_ORIGIN).hostname;
  newHeaders.set('Host', originHost);

  // Set X-Forwarded headers
  newHeaders.set('X-Forwarded-Host', originalHost);
  newHeaders.set('X-Forwarded-Proto', 'https');

  // Add user agent if missing
  if (!newHeaders.has('User-Agent')) {
    newHeaders.set('User-Agent', 'Cloudflare-Worker-Docs-Proxy/1.0');
  }

  return newHeaders;
}

/**
 * Rewrite content to fix asset paths
 * Converts omg-docs.pages.dev URLs to /docs/ URLs
 */
function rewriteContent(content: string, isHTML: boolean, isCSS: boolean, isJS: boolean, hostname: string): string {
  let rewritten = content;

  if (isHTML) {
    // Rewrite absolute URLs in HTML
    rewritten = rewritten
      // Fix href and src attributes
      .replace(/href="https?:\/\/[^"]*omg-docs\.pages\.dev([^"]*)"/g, `href="https://${hostname}/docs$1"`)
      .replace(/src="https?:\/\/[^"]*omg-docs\.pages\.dev([^"]*)"/g, `src="https://${hostname}/docs$1"`)
      // Fix base href if present
      .replace(/<base\s+href="[^"]*omg-docs\.pages\.dev([^"]*)"/g, `<base href="https://${hostname}/docs$1"`)
      // Fix meta tags
      .replace(/content="https?:\/\/[^"]*omg-docs\.pages\.dev([^"]*)"/g, `content="https://${hostname}/docs$1"`)
      // Fix JSON-LD and structured data
      .replace(/https?:\/\/[^"]*omg-docs\.pages\.dev/g, `https://${hostname}/docs`);
  }

  if (isCSS) {
    // Rewrite URLs in CSS url() functions
    rewritten = rewritten
      .replace(/url\(["']?https?:\/\/[^)"']*omg-docs\.pages\.dev([^)"']*)["']?\)/g, `url("https://${hostname}/docs$1")`)
      .replace(/url\(["']?\/([^)"']*)["']?\)/g, `url("/docs/$1")`);
  }

  if (isJS) {
    // Rewrite URLs in JavaScript strings (careful to not break code)
    rewritten = rewritten
      .replace(/"https?:\/\/[^"]*omg-docs\.pages\.dev([^"]*)"/g, `"https://${hostname}/docs$1"`)
      .replace(/'https?:\/\/[^']*omg-docs\.pages\.dev([^']*)'/g, `'https://${hostname}/docs$1'`);
  }

  return rewritten;
}

/**
 * Rewrite redirect URLs
 */
function rewriteUrl(url: string, hostname: string): string {
  try {
    const parsed = new URL(url);
    if (parsed.hostname.includes('omg-docs.pages.dev')) {
      return `https://${hostname}/docs${parsed.pathname}${parsed.search}${parsed.hash}`;
    }
    return url;
  } catch {
    // If URL parsing fails, return as-is
    return url;
  }
}

/**
 * Determine if resource should be cached
 */
function shouldCache(pathname: string, contentType: string): boolean {
  // Always cache static assets
  if (pathname.match(/\.(js|css|woff2?|ttf|eot|svg|png|jpg|jpeg|gif|webp|ico|pdf)$/i)) {
    return true;
  }

  // Cache HTML with shorter TTL
  if (contentType.includes('text/html')) {
    return true;
  }

  // Cache JSON API responses
  if (contentType.includes('application/json')) {
    return true;
  }

  return false;
}

/**
 * Get cache TTL based on content type and path
 */
function getCacheTtl(pathname: string, contentType: string): number {
  // Immutable assets (hashed filenames) - cache for 1 year
  if (pathname.match(/\.[a-f0-9]{8,}\.(js|css|woff2?|ttf|eot)$/i)) {
    return 31536000; // 1 year
  }

  // Images and fonts - cache for 7 days
  if (pathname.match(/\.(woff2?|ttf|eot|svg|png|jpg|jpeg|gif|webp|ico)$/i)) {
    return 604800; // 7 days
  }

  // Regular JS/CSS - cache for 1 day
  if (pathname.match(/\.(js|css)$/i)) {
    return 86400; // 1 day
  }

  // HTML - cache for 5 minutes (allows quick updates)
  if (contentType.includes('text/html')) {
    return 300; // 5 minutes
  }

  // JSON - cache for 1 minute
  if (contentType.includes('application/json')) {
    return 60; // 1 minute
  }

  // Default - cache for 1 hour
  return 3600; // 1 hour
}
