/**
 * OMG Router Worker
 * Routes /docs/* to the Docusaurus docs site
 * All other requests pass through to the main site
 */

export interface Env {
  MAIN_SITE: string;
  DOCS_SITE: string;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;

    // Route /docs requests to the docs site
    if (path === '/docs' || path.startsWith('/docs/')) {
      // Forward the full path including /docs prefix to the Pages deployment
      const docsUrl = new URL(path + url.search, env.DOCS_SITE);

      const response = await fetch(docsUrl.toString(), {
        method: request.method,
        headers: {
          'Accept': request.headers.get('Accept') || '*/*',
          'Accept-Encoding': request.headers.get('Accept-Encoding') || 'gzip, deflate, br',
          'Accept-Language': request.headers.get('Accept-Language') || 'en-US,en;q=0.9',
          'Cache-Control': request.headers.get('Cache-Control') || '',
          'Connection': 'keep-alive',
          'Host': new URL(env.DOCS_SITE).host,
          'User-Agent': request.headers.get('User-Agent') || 'Mozilla/5.0',
        },
        body: request.method !== 'GET' && request.method !== 'HEAD' ? request.body : undefined,
        redirect: 'follow',
      });

      // Clone response with modified headers
      const newHeaders = new Headers(response.headers);
      
      // Ensure proper caching
      if (!newHeaders.has('Cache-Control')) {
        newHeaders.set('Cache-Control', 'public, max-age=3600');
      }

      return new Response(response.body, {
        status: response.status,
        statusText: response.statusText,
        headers: newHeaders,
      });
    }

    // All other requests go to main site
    const mainUrl = new URL(path + url.search, env.MAIN_SITE);
    return fetch(mainUrl.toString(), {
      method: request.method,
      headers: {
        'Accept': request.headers.get('Accept') || '*/*',
        'Accept-Encoding': request.headers.get('Accept-Encoding') || 'gzip, deflate, br',
        'Accept-Language': request.headers.get('Accept-Language') || 'en-US,en;q=0.9',
        'Cache-Control': request.headers.get('Cache-Control') || '',
        'Connection': 'keep-alive',
        'Host': new URL(env.MAIN_SITE).host,
        'User-Agent': request.headers.get('User-Agent') || 'Mozilla/5.0',
      },
      body: request.method !== 'GET' && request.method !== 'HEAD' ? request.body : undefined,
      redirect: 'follow',
    });
  },
};
