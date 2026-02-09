import { APIEvent } from "@solidjs/start/server";
import { drizzle } from "drizzle-orm/d1";
import { eq, and } from "drizzle-orm";
import * as schema from "~/db/auth-schema";
import { createAuth, CloudflareEnv } from "~/lib/auth";

function getEnv(event: APIEvent): CloudflareEnv {
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
  };
}

/**
 * Sync license tier from external API (api.pyro1121.com) to D1 database
 * This endpoint is called automatically by the dashboard to ensure tier is up-to-date
 */
export async function POST(event: APIEvent) {
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

    console.log('[Sync License] Querying license for userId:', userId);

    const license = await db
      .select()
      .from(schema.license)
      .where(eq(schema.license.userId, userId))
      .limit(1)
      .get();

    console.log('[Sync License] License found:', license ? `id=${license.id}, tier=${license.tier}, licenseKey=${license.licenseKey}` : 'null');

    if (!license) {
      return new Response(JSON.stringify({ error: "No license found" }), {
        status: 404,
        headers: { "Content-Type": "application/json" },
      });
    }

    const externalApiResponse = await fetch("https://api.pyro1121.com/api/validate-license", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        license_key: license.licenseKey,
      }),
    });

    if (!externalApiResponse.ok) {
      console.error("[Sync License] External API error:", externalApiResponse.status);
      return new Response(JSON.stringify({ 
        error: "Failed to validate with external API",
        synced: false 
      }), {
        status: 500,
        headers: { "Content-Type": "application/json" },
      });
    }

    const externalData = await externalApiResponse.json();

    console.log('[Sync License] External API response:', { valid: externalData.valid, tier: externalData.tier, max_machines: externalData.max_machines });

    if (!externalData.valid) {
      return new Response(JSON.stringify({ 
        error: "License is not valid",
        synced: false 
      }), {
        status: 400,
        headers: { "Content-Type": "application/json" },
      });
    }

    const newTier = externalData.tier || "free";
    const maxMachines = externalData.max_machines || license.maxMachines;

    console.log('[Sync License] Comparing - DB tier:', license.tier, 'vs External tier:', newTier);

    if (license.tier !== newTier || license.maxMachines !== maxMachines) {
      console.log('[Sync License] Updating database: old_tier =', license.tier, ', new_tier =', newTier);
      
      await db
        .update(schema.license)
        .set({
          tier: newTier,
          maxMachines: maxMachines,
          updatedAt: new Date(),
        })
        .where(eq(schema.license.id, license.id))
        .run();

      console.log('[Sync License] Database updated successfully');

      // Sync machines from external API to auth-db
      const machinesSynced = await syncMachines(db, license.id, externalData.machines);

      return new Response(JSON.stringify({
        synced: true,
        old_tier: license.tier,
        new_tier: newTier,
        max_machines: maxMachines,
        machines_synced: machinesSynced,
      }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }

    console.log('[Sync License] No update needed - tiers match');

    // Sync machines from external API to auth-db
    const machinesSynced = await syncMachines(db, license.id, externalData.machines);

    return new Response(JSON.stringify({
      synced: true,
      message: "Already up to date",
      tier: newTier,
      machines_synced: machinesSynced,
    }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  } catch (error) {
    console.error("[Sync License] Error:", error);
    return new Response(
      JSON.stringify({
        error: "Internal server error",
        message: error instanceof Error ? error.message : "Unknown error",
        synced: false
      }),
      {
        status: 500,
        headers: { "Content-Type": "application/json" },
      }
    );
  }
}

/**
 * Sync machines from the external API (omg-licensing) to auth-db
 * Uses upsert logic: insert new machines, update existing ones
 */
async function syncMachines(
  db: ReturnType<typeof drizzle>,
  licenseId: string,
  machines?: Array<{
    machine_id: string;
    hostname?: string;
    os?: string;
    arch?: string;
    omg_version?: string;
    is_active: number;
    first_seen_at?: string;
    last_seen_at?: string;
  }>
): Promise<number> {
  if (!machines || machines.length === 0) return 0;

  let synced = 0;

  for (const m of machines) {
    try {
      // Check if machine already exists in auth-db
      const existing = await db
        .select({ id: schema.machine.id })
        .from(schema.machine)
        .where(
          and(
            eq(schema.machine.licenseId, licenseId),
            eq(schema.machine.machineId, m.machine_id)
          )
        )
        .limit(1)
        .get();

      const now = new Date();
      const firstSeen = m.first_seen_at ? new Date(m.first_seen_at) : now;
      const lastSeen = m.last_seen_at ? new Date(m.last_seen_at) : now;

      if (existing) {
        // Update existing machine
        await db
          .update(schema.machine)
          .set({
            hostname: m.hostname || null,
            os: m.os || null,
            arch: m.arch || null,
            omgVersion: m.omg_version || null,
            isActive: m.is_active === 1,
            lastSeenAt: lastSeen,
          })
          .where(eq(schema.machine.id, existing.id))
          .run();
      } else {
        // Insert new machine
        await db
          .insert(schema.machine)
          .values({
            id: crypto.randomUUID(),
            licenseId,
            machineId: m.machine_id,
            hostname: m.hostname || null,
            os: m.os || null,
            arch: m.arch || null,
            omgVersion: m.omg_version || null,
            isActive: m.is_active === 1,
            firstSeenAt: firstSeen,
            lastSeenAt: lastSeen,
          })
          .run();
      }
      synced++;
    } catch (err) {
      console.error(`[Sync Machines] Error syncing machine ${m.machine_id}:`, err);
    }
  }

  console.log(`[Sync Machines] Synced ${synced}/${machines.length} machines`);
  return synced;
}
