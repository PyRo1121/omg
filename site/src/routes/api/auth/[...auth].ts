import type { APIEvent } from "@solidjs/start/server";
import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";
import { drizzle } from "drizzle-orm/d1";

async function handleAuth(event: APIEvent): Promise<Response> {
  try {
    const cf = (event.nativeEvent as any).context?.cloudflare?.env;
    const db = drizzle(cf.DB);

    const auth = betterAuth({
      database: drizzleAdapter(db, {
        provider: "sqlite",
        usePlural: false,
      }),
      secret: cf.BETTER_AUTH_SECRET,
      baseURL: cf.BETTER_AUTH_URL,
      emailAndPassword: {
        enabled: true,
      },
      socialProviders: {
        github: {
          clientId: cf.GITHUB_CLIENT_ID,
          clientSecret: cf.GITHUB_CLIENT_SECRET,
        },
        google: {
          clientId: cf.GOOGLE_CLIENT_ID,
          clientSecret: cf.GOOGLE_CLIENT_SECRET,
        },
      },
    });

    return await auth.handler(event.request);
  } catch (error) {
    console.error("[AUTH ERROR]", error);
    return new Response(JSON.stringify({
      error: error instanceof Error ? error.message : String(error),
      stack: error instanceof Error ? error.stack : undefined,
    }), {
      status: 500,
      headers: { "Content-Type": "application/json" }
    });
  }
}

export const GET = handleAuth;
export const POST = handleAuth;
