import { APIEvent } from "@solidjs/start/server";
import { drizzle } from "drizzle-orm/d1";
import { eq } from "drizzle-orm";
import * as schema from "~/db/auth-schema";
import { createAuth, CloudflareEnv } from "~/lib/auth";

function getEnv(event: APIEvent): CloudflareEnv & { WORKERS_API_URL?: string; ADMIN_API_SECRET?: string } {
  const env = (event.nativeEvent as any).context?.cloudflare?.env;
  if (!env) throw new Error("Cloudflare environment not available");
  
  return {
    DB: env.DB,
    BETTER_AUTH_KV: env.BETTER_AUTH_KV,
    BETTER_AUTH_SECRET: env.BETTER_AUTH_SECRET,
    BETTER_AUTH_URL: env.BETTER_AUTH_URL,
    GITHUB_CLIENT_ID: env.GITHUB_CLIENT_ID,
    GITHUB_CLIENT_SECRET: env.GITHUB_CLIENT_SECRET,
    GOOGLE_CLIENT_ID: env.GOOGLE_CLIENT_ID,
    GOOGLE_CLIENT_SECRET: env.GOOGLE_CLIENT_SECRET,
    WORKERS_API_URL: env.WORKERS_API_URL || "https://api.pyro1121.com",
    ADMIN_API_SECRET: env.ADMIN_API_SECRET,
  };
}

export async function GET(event: APIEvent) {
  try {
    const env = getEnv(event);
    const auth = createAuth(env);
    
    const session = await auth.api.getSession({
      headers: event.request.headers,
    });

    if (!session?.user) {
      return new Response(JSON.stringify({ error: "Unauthorized" }), {
        status: 401,
        headers: { "Content-Type": "application/json" },
      });
    }

    const db = drizzle(env.DB, { schema });
    const userId = session.user.id;

    const userRecord = await db
      .select({ role: schema.user.role })
      .from(schema.user)
      .where(eq(schema.user.id, userId))
      .limit(1)
      .get();

    if (userRecord?.role !== 'admin') {
      return new Response(JSON.stringify({ error: "Forbidden - Admin access required" }), {
        status: 403,
        headers: { "Content-Type": "application/json" },
      });
    }

    const workersApiUrl = env.WORKERS_API_URL;
    const adminSecret = env.ADMIN_API_SECRET;

    if (!adminSecret) {
      console.error("[Auth Bridge] ADMIN_API_SECRET not configured");
      return new Response(JSON.stringify({ error: "Admin API not configured" }), {
        status: 500,
        headers: { "Content-Type": "application/json" },
      });
    }

    const response = await fetch(`${workersApiUrl}/api/admin/create-session`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Admin-Secret": adminSecret,
      },
      body: JSON.stringify({
        email: session.user.email,
        name: session.user.name,
        betterAuthUserId: userId,
      }),
    });

    if (!response.ok) {
      const errorText = await response.text();
      console.error("[Auth Bridge] Workers API error:", errorText);
      return new Response(JSON.stringify({ error: "Failed to create workers session" }), {
        status: 500,
        headers: { "Content-Type": "application/json" },
      });
    }

    const data = await response.json() as { token: string; expiresAt: string };

    return new Response(JSON.stringify({
      token: data.token,
      expiresAt: data.expiresAt,
    }), {
      status: 200,
      headers: { 
        "Content-Type": "application/json",
        "Cache-Control": "private, no-cache, no-store, must-revalidate",
      },
    });
  } catch (error) {
    console.error("[Auth Bridge] Error:", error);
    return new Response(JSON.stringify({ 
      error: "Internal server error",
      message: error instanceof Error ? error.message : "Unknown error"
    }), {
      status: 500,
      headers: { "Content-Type": "application/json" },
    });
  }
}
