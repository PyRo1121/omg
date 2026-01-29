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

    const body = await event.request.json();
    console.log("[TEST EMAIL] Body:", JSON.stringify(body));
    
    const auth = createAuth(env);
    console.log("[TEST EMAIL] Auth created");
    
    const testRequest = new Request("https://pyro1121.com/api/auth/sign-up/email", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(body),
    });
    
    console.log("[TEST EMAIL] Calling handler...");
    const response = await auth.handler(testRequest);
    console.log("[TEST EMAIL] Response status:", response.status);
    
    const responseText = await response.text();
    console.log("[TEST EMAIL] Response body:", responseText);
    
    return Response.json({
      success: true,
      status: response.status,
      body: responseText,
      requestBody: body,
    });
  } catch (error) {
    console.error("[TEST EMAIL ERROR]", error);
    return Response.json({
      error: error instanceof Error ? error.message : String(error),
      stack: error instanceof Error ? error.stack : undefined,
      name: error?.constructor?.name,
    }, { status: 500 });
  }
}
