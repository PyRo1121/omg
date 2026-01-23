export interface Env {
  BUCKET: R2Bucket;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;

    // Endpoint: GET /latest-version
    // Returns the semantic version string of the latest stable release (e.g. "0.1.75")
    if (path === '/latest-version') {
      const object = await env.BUCKET.get('latest-version');
      if (!object) {
        // Fallback for initial setup
        return new Response('0.1.75', {
          headers: { 'Content-Type': 'text/plain' },
        });
      }
      return new Response(object.body, {
        headers: { 'Content-Type': 'text/plain' },
      });
    }

    // Endpoint: GET /download/:filename
    // Serves release artifacts (binaries, sigs, sha256)
    if (path.startsWith('/download/')) {
      const filename = path.replace('/download/', '');
      const object = await env.BUCKET.get(filename);

      if (!object) {
        return new Response('Not Found', { status: 404 });
      }

      const headers = new Headers();
      object.writeHttpMetadata(headers);
      headers.set('etag', object.httpEtag);

      return new Response(object.body, {
        headers,
      });
    }

    // Default: 404
    return new Response('Not Found', { status: 404 });
  },
};
