import { Env, errorResponse } from '../api';

export async function handleBinaryDownload(request: Request, env: Env): Promise<Response> {
  const url = new URL(request.url);
  const platform = url.pathname.split('/').pop();
  
  if (!platform) {
    return errorResponse('Platform not specified', 400);
  }

  const filename = `omg-${platform}.tar.gz`;
  const object = await env.ASSETS.get(`binaries/${filename}`);
  
  if (!object) {
    return errorResponse('Binary not found for platform: ' + platform, 404);
  }

  const range = request.headers.get('Range');
  
  if (range) {
    const parts = range.replace(/bytes=/, '').split('-');
    const start = parseInt(parts[0], 10);
    const end = parts[1] ? parseInt(parts[1], 10) : object.size - 1;
    
    if (isNaN(start) || isNaN(end) || start >= object.size || end >= object.size || start > end) {
      return new Response('Invalid range', { status: 416 });
    }

    const rangeObject = await env.ASSETS.get(`binaries/${filename}`, {
      range: { offset: start, length: end - start + 1 }
    });

    if (!rangeObject) {
      return errorResponse('Range request failed', 500);
    }

    return new Response(rangeObject.body, {
      status: 206,
      headers: {
        'Content-Type': 'application/gzip',
        'Content-Disposition': `attachment; filename="${filename}"`,
        'Content-Range': `bytes ${start}-${end}/${object.size}`,
        'Content-Length': (end - start + 1).toString(),
        'Accept-Ranges': 'bytes',
        'Cache-Control': 'public, max-age=31536000, immutable',
        'ETag': object.httpEtag || object.etag,
      }
    });
  }

  return new Response(object.body, {
    headers: {
      'Content-Type': 'application/gzip',
      'Content-Disposition': `attachment; filename="${filename}"`,
      'Content-Length': object.size.toString(),
      'Accept-Ranges': 'bytes',
      'Cache-Control': 'public, max-age=31536000, immutable',
      'ETag': object.httpEtag || object.etag,
    }
  });
}
