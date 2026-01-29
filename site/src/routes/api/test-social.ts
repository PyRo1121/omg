import type { APIEvent } from "@solidjs/start/server";
import { createAuth } from "~/lib/auth";

export async function POST(event: APIEvent) {
  try {
    const cf = (event.nativeEvent as any).context?.cloudflare?.env;
    const env = {
      DB: cf.DB,
      BETTER_AUTH_KV: cf.BETTER_AUTH_KV,
      BETTER_AUTH_SECRET: cf.BETTER_AUTH_SECRET,
      BETTER_AUTH_URL: cf.BETTER_AUTH_URL,
      GITHUB_CLIENT_ID: cf.GITHUB_CLIENT_ID,
      GITHUB_CLIENT_SECRET: cf.GITHUB_CLIENT_SECRET,
      GOOGLE_CLIENT_ID: cf.GOOGLE_CLIENT_ID,
      GOOGLE_CLIENT_SECRET: cf.GOOGLE_CLIENT_SECRET,
    };

    const auth = createAuth(env);
    
    const body = await event.request.json();
    
    const testRequest = new Request("https://pyro1121.com/api/auth/sign-in/social", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
    });
    
    const response = await auth.handler(testRequest);
    const responseBody = await response.text();
    
    return Response.json({
      status: response.status,
      body: responseBody,
      provider: body.provider,
    });
  } catch (error) {
    return Response.json({
      error: error instanceof Error ? error.message : String(error),
      stack: error instanceof Error ? error.stack : undefined,
    }, { status: 500 });
  }
}
