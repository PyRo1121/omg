import { Env, errorResponse } from '../api';

export async function handleImageOptimization(request: Request, env: Env): Promise<Response> {
  const url = new URL(request.url);
  const imagePath = url.pathname.replace('/img/', '');
  
  if (!imagePath) {
    return errorResponse('Image path not specified', 400);
  }

  const cacheKey = new URL(request.url);
  const cache = caches.default;
  
  let cachedResponse = await cache.match(cacheKey);
  if (cachedResponse) {
    return new Response(cachedResponse.body, {
      headers: {
        ...Object.fromEntries(cachedResponse.headers),
        'X-Cache': 'HIT'
      }
    });
  }

  const object = await env.ASSETS.get(`images/${imagePath}`);
  
  if (!object) {
    return errorResponse('Image not found: ' + imagePath, 404);
  }

  const headers = new Headers();
  headers.set('Content-Type', getContentType(imagePath));
  headers.set('Cache-Control', 'public, max-age=31536000, immutable');
  headers.set('Vary', 'Accept-Encoding');
  headers.set('X-Cache', 'MISS');
  
  if (object.httpEtag) {
    headers.set('ETag', object.httpEtag);
  }

  const response = new Response(object.body, { headers });

  await cache.put(cacheKey, response.clone());
  
  return response;
}

function getContentType(path: string): string {
  if (path.endsWith('.png')) return 'image/png';
  if (path.endsWith('.jpg') || path.endsWith('.jpeg')) return 'image/jpeg';
  if (path.endsWith('.webp')) return 'image/webp';
  if (path.endsWith('.avif')) return 'image/avif';
  if (path.endsWith('.gif')) return 'image/gif';
  if (path.endsWith('.svg')) return 'image/svg+xml';
  
  return 'image/png';
}
