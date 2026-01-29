import { APIEvent } from "@solidjs/start/server";
import { drizzle } from "drizzle-orm/d1";
import { eq } from "drizzle-orm";
import * as schema from "~/db/auth-schema";

function getEnv(event: APIEvent) {
  const env = (event.nativeEvent as any).context?.cloudflare?.env;
  if (!env) throw new Error("Environment not available");
  return env;
}

export async function POST(event: APIEvent) {
  try {
    const env = getEnv(event);
    const db = drizzle(env.DB, { schema });
    const body = await event.request.json();

    if (!body.license_key) {
      return new Response(JSON.stringify({ error: "License key required" }), {
        status: 400,
        headers: { "Content-Type": "application/json" },
      });
    }

    const license = await db
      .select({
        id: schema.license.id,
        tier: schema.license.tier,
        status: schema.license.status,
        maxMachines: schema.license.maxMachines,
        expiresAt: schema.license.expiresAt,
      })
      .from(schema.license)
      .where(eq(schema.license.licenseKey, body.license_key))
      .limit(1)
      .get();

    if (!license) {
      return new Response(JSON.stringify({ 
        valid: false, 
        error: "License not found" 
      }), {
        status: 404,
        headers: { "Content-Type": "application/json" },
      });
    }

    if (license.status !== "active") {
      return new Response(
        JSON.stringify({
          valid: false,
          error: `License is ${license.status}`,
          status: license.status,
        }),
        {
          status: 403,
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    if (license.expiresAt && new Date(license.expiresAt) < new Date()) {
      await db
        .update(schema.license)
        .set({ status: "expired" })
        .where(eq(schema.license.id, license.id))
        .run();

      return new Response(
        JSON.stringify({
          valid: false,
          error: "License has expired",
          expired_at: license.expiresAt,
        }),
        {
          status: 403,
          headers: { "Content-Type": "application/json" },
        }
      );
    }

    const machines = await db
      .select()
      .from(schema.machine)
      .where(eq(schema.machine.licenseId, license.id))
      .all();

    return new Response(
      JSON.stringify({
        valid: true,
        tier: license.tier,
        max_machines: license.maxMachines,
        current_machines: machines.length,
        expires_at: license.expiresAt,
      }),
      {
        status: 200,
        headers: { 
          "Content-Type": "application/json",
          "Cache-Control": "private, max-age=300"
        },
      }
    );
  } catch (error) {
    console.error("[CLI Validate License] Error:", error);
    return new Response(
      JSON.stringify({
        valid: false,
        error: "Internal server error",
        message: error instanceof Error ? error.message : "Unknown error",
      }),
      {
        status: 500,
        headers: { "Content-Type": "application/json" },
      }
    );
  }
}
