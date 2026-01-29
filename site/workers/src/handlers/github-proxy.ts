import { Env, jsonResponse, errorResponse } from '../api';

interface GitHubCommitActivity {
  days: number[];
  total: number;
  week: number;
}

const CACHE_TTL = 120;
const STALE_TTL = 3600;

export async function handleGitHubProxy(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
  if (request.method === 'OPTIONS') {
    return new Response(null, {
      headers: {
        'Access-Control-Allow-Origin': '*',
        'Access-Control-Allow-Methods': 'GET, OPTIONS',
        'Access-Control-Allow-Headers': 'Content-Type',
        'Access-Control-Max-Age': '86400',
      }
    });
  }

  if (request.method !== 'GET') {
    return errorResponse('Method not allowed', 405);
  }

  const cache = caches.default;
  const cacheKey = new Request(request.url, request);

  let cachedResponse = await cache.match(cacheKey);
  
  if (cachedResponse) {
    const age = parseInt(cachedResponse.headers.get('Age') || '0');
    
    if (age < CACHE_TTL) {
      return new Response(cachedResponse.body, {
        headers: { 
          ...Object.fromEntries(cachedResponse.headers),
          'X-Cache': 'HIT',
          'X-Cache-Age': age.toString(),
          'Access-Control-Allow-Origin': '*'
        }
      });
    }
    
    if (age < STALE_TTL) {
      ctx.waitUntil(refreshCache(env, cache, cacheKey));
      
      return new Response(cachedResponse.body, {
        headers: { 
          ...Object.fromEntries(cachedResponse.headers),
          'X-Cache': 'STALE',
          'X-Cache-Age': age.toString(),
          'Access-Control-Allow-Origin': '*'
        }
      });
    }
  }

  return await refreshCache(env, cache, cacheKey);
}

async function refreshCache(env: Env, cache: Cache, cacheKey: Request): Promise<Response> {
  const ghResponse = await fetch(
    'https://api.github.com/repos/PyRo1121/omg/stats/commit_activity',
    { 
      headers: { 
        'Accept': 'application/vnd.github.v3+json',
        'User-Agent': 'OMG-Package-Manager-Site/1.0'
      } 
    }
  );

  const remaining = ghResponse.headers.get('X-RateLimit-Remaining');
  if (remaining && parseInt(remaining) < 10) {
    console.warn(`GitHub rate limit low: ${remaining} requests remaining`);
  }

  if (ghResponse.status === 202) {
    return new Response(JSON.stringify({ 
      computing: true, 
      message: 'GitHub is computing statistics. Please try again in 60 seconds.' 
    }), {
      status: 202,
      headers: {
        'Content-Type': 'application/json',
        'Cache-Control': 'no-cache',
        'Retry-After': '60',
        'Access-Control-Allow-Origin': '*',
        'X-GitHub-Status': 'computing'
      }
    });
  }

  if (!ghResponse.ok) {
    console.error(`GitHub API error: ${ghResponse.status}`);
    return errorResponse(`GitHub API error: ${ghResponse.status}`, ghResponse.status);
  }

  const data: GitHubCommitActivity[] = await ghResponse.json();
  const responseBody = JSON.stringify(data);
  
  const response = new Response(responseBody, {
    headers: {
      'Content-Type': 'application/json',
      'Cache-Control': `public, max-age=${CACHE_TTL}, stale-while-revalidate=${STALE_TTL}`,
      'Access-Control-Allow-Origin': '*',
      'X-Cache': 'MISS',
      'X-RateLimit-Remaining': remaining || 'unknown'
    }
  });

  await cache.put(cacheKey, response.clone());
  
  return response;
}
