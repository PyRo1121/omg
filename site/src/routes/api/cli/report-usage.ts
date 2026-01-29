import { APIEvent } from "@solidjs/start/server";
import { drizzle } from "drizzle-orm/d1";
import { eq, and } from "drizzle-orm";
import * as schema from "~/db/auth-schema";
import type { CLITelemetryReport } from "~/types/telemetry";
import { ACHIEVEMENTS, checkAchievementProgress } from "~/lib/achievements";

function getEnv(event: APIEvent) {
  const env = (event.nativeEvent as any).context?.cloudflare?.env;
  if (!env) throw new Error("Environment not available");
  return env;
}

async function updateAchievements(
  db: ReturnType<typeof drizzle>,
  userId: string,
  stats: {
    commands_run: number;
    packages_searched: number;
    packages_installed: number;
    runtimes_switched: number;
    sbom_generated: number;
    vulnerabilities_found: number;
    time_saved_ms: number;
  },
) {
  for (const achievement of ACHIEVEMENTS) {
    const progress = checkAchievementProgress(achievement, stats);
    const isUnlocked = progress >= 100;

    const existing = await db
      .select()
      .from(schema.userAchievement)
      .where(
        and(
          eq(schema.userAchievement.userId, userId),
          eq(schema.userAchievement.achievementId, achievement.id)
        )
      )
      .limit(1)
      .get();

    if (existing) {
      if (!existing.isUnlocked && isUnlocked) {
        await db
          .update(schema.userAchievement)
          .set({
            progress: Math.round(progress),
            isUnlocked: true,
            unlockedAt: new Date(),
          })
          .where(eq(schema.userAchievement.id, existing.id))
          .run();
      } else {
        await db
          .update(schema.userAchievement)
          .set({ progress: Math.round(progress) })
          .where(eq(schema.userAchievement.id, existing.id))
          .run();
      }
    } else {
      await db
        .insert(schema.userAchievement)
        .values({
          id: crypto.randomUUID(),
          userId: userId,
          achievementId: achievement.id,
          progress: Math.round(progress),
          isUnlocked: isUnlocked,
          unlockedAt: isUnlocked ? new Date() : null,
          createdAt: new Date(),
        })
        .run();
    }
  }
}

export async function POST(event: APIEvent) {
  try {
    const env = getEnv(event);
    const db = drizzle(env.DB, { schema });
    const body = (await event.request.json()) as CLITelemetryReport;

    const license = await db
      .select()
      .from(schema.license)
      .where(eq(schema.license.licenseKey, body.license_key))
      .limit(1)
      .get();

    if (!license || license.status !== "active") {
      return new Response(JSON.stringify({ error: "Invalid or inactive license" }), {
        status: 401,
        headers: { "Content-Type": "application/json" },
      });
    }

    let machine = await db
      .select()
      .from(schema.machine)
      .where(
        and(
          eq(schema.machine.licenseId, license.id),
          eq(schema.machine.machineId, body.machine_id)
        )
      )
      .limit(1)
      .get();

    const now = new Date();

    if (!machine) {
      const machines = await db
        .select()
        .from(schema.machine)
        .where(eq(schema.machine.licenseId, license.id))
        .all();

      if (machines.length >= license.maxMachines) {
        return new Response(
          JSON.stringify({
            error: "Maximum machines reached",
            current: machines.length,
            max: license.maxMachines,
          }),
          {
            status: 403,
            headers: { "Content-Type": "application/json" },
          }
        );
      }

      await db
        .insert(schema.machine)
        .values({
          id: crypto.randomUUID(),
          licenseId: license.id,
          machineId: body.machine_id,
          hostname: body.hostname || null,
          os: body.os || null,
          arch: body.arch || null,
          omgVersion: body.omg_version || null,
          isActive: true,
          firstSeenAt: now,
          lastSeenAt: now,
          createdAt: now,
        })
        .run();
    } else {
      await db
        .update(schema.machine)
        .set({
          lastSeenAt: now,
          omgVersion: body.omg_version || machine.omgVersion,
          hostname: body.hostname || machine.hostname,
          os: body.os || machine.os,
          arch: body.arch || machine.arch,
          isActive: true,
        })
        .where(eq(schema.machine.id, machine.id))
        .run();
    }

    const today = now.toISOString().split("T")[0];
    const existingUsage = await db
      .select()
      .from(schema.usageDaily)
      .where(
        and(
          eq(schema.usageDaily.licenseId, license.id),
          eq(schema.usageDaily.date, today)
        )
      )
      .limit(1)
      .get();

    if (existingUsage) {
      await db
        .update(schema.usageDaily)
        .set({
          commandsRun: existingUsage.commandsRun + (body.commands_run || 0),
          packagesInstalled:
            existingUsage.packagesInstalled + (body.packages_installed || 0),
          packagesSearched:
            existingUsage.packagesSearched + (body.packages_searched || 0),
          runtimesSwitched:
            existingUsage.runtimesSwitched + (body.runtimes_switched || 0),
          sbomGenerated: existingUsage.sbomGenerated + (body.sbom_generated || 0),
          vulnerabilitiesFound:
            existingUsage.vulnerabilitiesFound + (body.vulnerabilities_found || 0),
          timeSavedMs: existingUsage.timeSavedMs + (body.time_saved_ms || 0),
          updatedAt: now,
        })
        .where(eq(schema.usageDaily.id, existingUsage.id))
        .run();
    } else {
      await db
        .insert(schema.usageDaily)
        .values({
          id: crypto.randomUUID(),
          licenseId: license.id,
          date: today,
          commandsRun: body.commands_run || 0,
          packagesInstalled: body.packages_installed || 0,
          packagesSearched: body.packages_searched || 0,
          runtimesSwitched: body.runtimes_switched || 0,
          sbomGenerated: body.sbom_generated || 0,
          vulnerabilitiesFound: body.vulnerabilities_found || 0,
          timeSavedMs: body.time_saved_ms || 0,
          createdAt: now,
          updatedAt: now,
        })
        .run();
    }

    const thirtyDaysAgo = new Date();
    thirtyDaysAgo.setDate(thirtyDaysAgo.getDate() - 30);
    const dateStr = thirtyDaysAgo.toISOString().split("T")[0];

    const totalStats = await db
      .select()
      .from(schema.usageDaily)
      .where(
        and(
          eq(schema.usageDaily.licenseId, license.id),
          eq(schema.usageDaily.date, dateStr)
        )
      )
      .all();

    const stats = {
      commands_run: totalStats.reduce((sum, s) => sum + s.commandsRun, 0),
      packages_searched: totalStats.reduce((sum, s) => sum + s.packagesSearched, 0),
      packages_installed: totalStats.reduce((sum, s) => sum + s.packagesInstalled, 0),
      runtimes_switched: totalStats.reduce((sum, s) => sum + s.runtimesSwitched, 0),
      sbom_generated: totalStats.reduce((sum, s) => sum + s.sbomGenerated, 0),
      vulnerabilities_found: totalStats.reduce((sum, s) => sum + s.vulnerabilitiesFound, 0),
      time_saved_ms: totalStats.reduce((sum, s) => sum + s.timeSavedMs, 0),
    };

    await updateAchievements(db, license.userId, stats);

    return new Response(JSON.stringify({ success: true }), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  } catch (error) {
    console.error("[CLI Report Usage] Error:", error);
    return new Response(
      JSON.stringify({
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
