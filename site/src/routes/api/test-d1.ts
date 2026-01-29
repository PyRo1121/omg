import type { APIEvent } from "@solidjs/start/server";
import { drizzle } from "drizzle-orm/d1";
import { sql } from "drizzle-orm";

export async function GET(event: APIEvent) {
  try {
    const cf = (event.nativeEvent as any).context?.cloudflare?.env;
    
    if (!cf?.DB) {
      return new Response(JSON.stringify({ error: "No DB binding" }), {
        status: 500,
        headers: { "Content-Type": "application/json" }
      });
    }

    const db = drizzle(cf.DB);
    const result = await db.run(sql`SELECT name FROM sqlite_master WHERE type='table'`);
    
    return new Response(JSON.stringify({ 
      success: true,
      tables: result.rows 
    }), {
      headers: { "Content-Type": "application/json" }
    });
  } catch (error) {
    return new Response(JSON.stringify({ 
      error: error instanceof Error ? error.message : String(error),
      stack: error instanceof Error ? error.stack : undefined
    }), {
      status: 500,
      headers: { "Content-Type": "application/json" }
    });
  }
}
