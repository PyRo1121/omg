import { APIEvent } from "@solidjs/start/server";
import { createAuth, CloudflareEnv } from "~/lib/auth";

function getEnv(event: APIEvent): CloudflareEnv {
  const cf = (event.nativeEvent as any).context?.cloudflare?.env;
  
  if (!cf?.DB) {
    throw new Error("D1 database binding not found. Ensure 'DB' is configured in wrangler.toml");
  }
  
  if (!cf?.BETTER_AUTH_KV) {
    throw new Error("KV namespace binding not found. Ensure 'BETTER_AUTH_KV' is configured in wrangler.toml");
  }

  return {
    DB: cf.DB,
    BETTER_AUTH_KV: cf.BETTER_AUTH_KV,
    BETTER_AUTH_SECRET: cf.BETTER_AUTH_SECRET || process.env.BETTER_AUTH_SECRET || "dev-secret-change-me",
    BETTER_AUTH_URL: cf.BETTER_AUTH_URL || process.env.BETTER_AUTH_URL || "http://localhost:3000",
    GITHUB_CLIENT_ID: cf.GITHUB_CLIENT_ID || process.env.GITHUB_CLIENT_ID,
    GITHUB_CLIENT_SECRET: cf.GITHUB_CLIENT_SECRET || process.env.GITHUB_CLIENT_SECRET,
    GOOGLE_CLIENT_ID: cf.GOOGLE_CLIENT_ID || process.env.GOOGLE_CLIENT_ID,
    GOOGLE_CLIENT_SECRET: cf.GOOGLE_CLIENT_SECRET || process.env.GOOGLE_CLIENT_SECRET,
  };
}

export async function GET(event: APIEvent) {
  const env = getEnv(event);
  const auth = createAuth(env);
  return auth.handler(event.request);
}

export async function POST(event: APIEvent) {
  const env = getEnv(event);
  const auth = createAuth(env);
  return auth.handler(event.request);
}
