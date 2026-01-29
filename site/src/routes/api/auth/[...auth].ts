import type { APIEvent } from "@solidjs/start/server";
import { createAuth, CloudflareEnv } from "~/lib/auth";

function getEnv(event: APIEvent): CloudflareEnv {
  const cf = (event.nativeEvent as any).context?.cloudflare?.env;
  
  if (!cf?.DB) {
    throw new Error("D1 database binding not found");
  }
  
  if (!cf?.BETTER_AUTH_KV) {
    throw new Error("KV namespace binding not found");
  }

  return {
    DB: cf.DB,
    BETTER_AUTH_KV: cf.BETTER_AUTH_KV,
    BETTER_AUTH_SECRET: cf.BETTER_AUTH_SECRET,
    BETTER_AUTH_URL: cf.BETTER_AUTH_URL,
    GITHUB_CLIENT_ID: cf.GITHUB_CLIENT_ID,
    GITHUB_CLIENT_SECRET: cf.GITHUB_CLIENT_SECRET,
    GOOGLE_CLIENT_ID: cf.GOOGLE_CLIENT_ID,
    GOOGLE_CLIENT_SECRET: cf.GOOGLE_CLIENT_SECRET,
  };
}

async function handleAuth(event: APIEvent): Promise<Response> {
  const corsHeaders = {
    "Access-Control-Allow-Origin": "https://pyro1121.com",
    "Access-Control-Allow-Methods": "GET, POST, PUT, PATCH, DELETE, OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type, Authorization",
    "Access-Control-Allow-Credentials": "true",
  };

  if (event.request.method === "OPTIONS") {
    return new Response(null, { status: 204, headers: corsHeaders });
  }

  try {
    const env = getEnv(event);
    const auth = createAuth(env);
    
    const response = await auth.handler(event.request);
    
    const newHeaders = new Headers(response.headers);
    Object.entries(corsHeaders).forEach(([key, value]) => {
      newHeaders.set(key, value);
    });
    
    return new Response(response.body, {
      status: response.status,
      statusText: response.statusText,
      headers: newHeaders,
    });
  } catch (error) {
    console.error("[AUTH ERROR]", error);
    
    return new Response(JSON.stringify({
      error: error instanceof Error ? error.message : String(error),
      stack: error instanceof Error ? error.stack : undefined,
    }), {
      status: 500,
      headers: { ...corsHeaders, "Content-Type": "application/json" }
    });
  }
}

export const GET = handleAuth;
export const POST = handleAuth;
export const PUT = handleAuth;
export const PATCH = handleAuth;
export const DELETE = handleAuth;
export const OPTIONS = handleAuth;
