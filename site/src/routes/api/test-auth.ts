import type { APIEvent } from "@solidjs/start/server";

export async function GET(event: APIEvent) {
  try {
    const cf = (event.nativeEvent as any).context?.cloudflare?.env;
    
    const step1 = "Getting env";
    if (!cf?.DB) {
      return Response.json({ error: "No DB", step: step1 });
    }

    const step2 = "Importing drizzle";
    const { drizzle } = await import("drizzle-orm/d1");
    
    const step3 = "Creating drizzle instance";
    const db = drizzle(cf.DB);

    const step4 = "Importing better-auth";
    const { betterAuth } = await import("better-auth");
    
    const step5 = "Importing drizzleAdapter";
    const { drizzleAdapter } = await import("better-auth/adapters/drizzle");

    const step6 = "Creating auth";
    const auth = betterAuth({
      database: drizzleAdapter(db, {
        provider: "sqlite",
        usePlural: false,
      }),
      secret: cf.BETTER_AUTH_SECRET || "test-secret",
      baseURL: cf.BETTER_AUTH_URL || "https://pyro1121.com",
    });

    const step7 = "Auth created successfully";
    
    return Response.json({ 
      success: true, 
      step: step7,
      hasHandler: typeof auth.handler === "function"
    });
  } catch (error) {
    return Response.json({ 
      error: error instanceof Error ? error.message : String(error),
      stack: error instanceof Error ? error.stack : undefined
    }, { status: 500 });
  }
}
