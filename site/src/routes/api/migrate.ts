import { APIEvent } from "@solidjs/start/server";
import { getMigrations } from "better-auth/db";
import { CloudflareEnv } from "~/lib/auth";

export async function GET(event: APIEvent) {
  const cf = (event.nativeEvent as any).context?.cloudflare?.env;
  
  if (!cf?.DB) {
    return new Response(JSON.stringify({ error: "D1 database binding not found" }), { 
      status: 500,
      headers: { "Content-Type": "application/json" }
    });
  }
  
  if (!cf?.BETTER_AUTH_KV) {
    return new Response(JSON.stringify({ error: "KV namespace binding not found" }), { 
      status: 500,
      headers: { "Content-Type": "application/json" }
    });
  }

  const env: CloudflareEnv = {
    DB: cf.DB,
    BETTER_AUTH_KV: cf.BETTER_AUTH_KV,
    BETTER_AUTH_SECRET: cf.BETTER_AUTH_SECRET || process.env.BETTER_AUTH_SECRET || "dev-secret-change-me",
    BETTER_AUTH_URL: cf.BETTER_AUTH_URL || process.env.BETTER_AUTH_URL || "http://localhost:3000",
    GITHUB_CLIENT_ID: cf.GITHUB_CLIENT_ID || process.env.GITHUB_CLIENT_ID,
    GITHUB_CLIENT_SECRET: cf.GITHUB_CLIENT_SECRET || process.env.GITHUB_CLIENT_SECRET,
    GOOGLE_CLIENT_ID: cf.GOOGLE_CLIENT_ID || process.env.GOOGLE_CLIENT_ID,
    GOOGLE_CLIENT_SECRET: cf.GOOGLE_CLIENT_SECRET || process.env.GOOGLE_CLIENT_SECRET,
  };
  
  // Build the auth config matching what createAuth uses
  const authConfig = {
    database: env.DB,
    secret: env.BETTER_AUTH_SECRET,
    baseURL: env.BETTER_AUTH_URL,
    secondaryStorage: {
      get: async (key: string) => {
        const value = await env.BETTER_AUTH_KV.get(key);
        return value ?? null;
      },
      set: async (key: string, value: string, ttl?: number) => {
        await env.BETTER_AUTH_KV.put(key, value, ttl ? { expirationTtl: ttl } : undefined);
      },
      delete: async (key: string) => {
        await env.BETTER_AUTH_KV.delete(key);
      },
    },
    emailAndPassword: {
      enabled: true,
    },
    socialProviders: {
      ...(env.GITHUB_CLIENT_ID && env.GITHUB_CLIENT_SECRET
        ? {
            github: {
              clientId: env.GITHUB_CLIENT_ID,
              clientSecret: env.GITHUB_CLIENT_SECRET,
            },
          }
        : {}),
      ...(env.GOOGLE_CLIENT_ID && env.GOOGLE_CLIENT_SECRET
        ? {
            google: {
              clientId: env.GOOGLE_CLIENT_ID,
              clientSecret: env.GOOGLE_CLIENT_SECRET,
            },
          }
        : {}),
    },
  };
  
  try {
    const { toBeCreated, toBeAdded, runMigrations } = await getMigrations(authConfig);

    if (toBeCreated.length === 0 && toBeAdded.length === 0) {
      return new Response(JSON.stringify({ 
        message: "No migrations needed",
        status: "up-to-date"
      }), { 
        status: 200,
        headers: { "Content-Type": "application/json" }
      });
    }

    await runMigrations();

    return new Response(JSON.stringify({
      message: "Migrations completed successfully",
      tablesCreated: toBeCreated.map(t => t.table),
      tablesUpdated: toBeAdded.map(t => t.table)
    }), { 
      status: 200,
      headers: { "Content-Type": "application/json" }
    });
  } catch (error) {
    console.error("Migration error:", error);
    return new Response(JSON.stringify({ 
      error: error instanceof Error ? error.message : "Migration failed" 
    }), { 
      status: 500,
      headers: { "Content-Type": "application/json" }
    });
  }
}

// Also support POST for curl -X POST
export async function POST(event: APIEvent) {
  return GET(event);
}
