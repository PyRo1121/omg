import type { APIEvent } from "@solidjs/start/server";
import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";
import { drizzle } from "drizzle-orm/d1";

export async function GET(event: APIEvent) {
  try {
    const cf = (event.nativeEvent as any).context?.cloudflare?.env;
    
    if (!cf?.DB) {
      return Response.json({ error: "No DB" }, { status: 500 });
    }

    const db = drizzle(cf.DB);

    const auth = betterAuth({
      database: drizzleAdapter(db, {
        provider: "sqlite",
        usePlural: false,
      }),
      secret: cf.BETTER_AUTH_SECRET || "test-secret",
      baseURL: "https://pyro1121.com",
    });

    const testRequest = new Request("https://pyro1121.com/api/auth/get-session", {
      method: "GET",
      headers: event.request.headers,
    });

    const response = await auth.handler(testRequest);
    const body = await response.text();
    
    return Response.json({ 
      success: true,
      status: response.status,
      body: body
    });
  } catch (error) {
    return Response.json({ 
      error: error instanceof Error ? error.message : String(error),
      stack: error instanceof Error ? error.stack : undefined
    }, { status: 500 });
  }
}
