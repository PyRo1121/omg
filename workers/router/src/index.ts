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
      // Remove /docs prefix for the docs site (it expects paths at root)
      const docsPath = path === '/docs' ? '/' : path.replace(/^\/docs/, '');
      const docsUrl = new URL(docsPath + url.search, env.DOCS_SITE);

      const response = await fetch(docsUrl.toString(), {
        method: request.method,
        headers: request.headers,
        body: request.method !== 'GET' && request.method !== 'HEAD' ? request.body : undefined,
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
      headers: request.headers,
      body: request.method !== 'GET' && request.method !== 'HEAD' ? request.body : undefined,
    });
  },
};
