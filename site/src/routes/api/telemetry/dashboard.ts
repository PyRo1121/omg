import { APIEvent } from "@solidjs/start/server";
import { drizzle } from "drizzle-orm/d1";
import { eq, desc, gte, sql, and } from "drizzle-orm";
import * as schema from "~/db/auth-schema";
import { createAuth, CloudflareEnv } from "~/lib/auth";
import { ACHIEVEMENTS, checkAchievementProgress, getIconByName } from "~/lib/achievements";
import type { TelemetryDashboardResponse } from "~/types/telemetry";

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

function getTierFeatures(tier: string): string[] {
  const features: Record<string, string[]> = {
    free: ["1 machine", "Basic telemetry", "Community support"],
    team: ["25 machines", "Advanced analytics", "Team dashboard", "Email support"],
    enterprise: ["Unlimited machines", "Custom integrations", "Dedicated support", "SLA"],
  };
  return features[tier] || features.free;
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

    console.log('[Dashboard API] Querying license for userId:', userId);

    let license = await db
      .select()
      .from(schema.license)
      .where(eq(schema.license.userId, userId))
      .limit(1)
      .get();

    console.log('[Dashboard API] License found:', license ? `id=${license.id}, tier=${license.tier}` : 'null');

    if (!license) {
      console.log('[Dashboard API] No license found, creating new "free" license');
      const licenseKey = crypto.randomUUID();
      const licenseId = crypto.randomUUID();
      
      await db.insert(schema.license).values({
        id: licenseId,
        userId: userId,
        licenseKey: licenseKey,
        tier: "free",
        status: "active",
        maxMachines: 1,
        expiresAt: null,
        createdAt: new Date(),
        updatedAt: new Date(),
      }).run();

      license = await db
        .select()
        .from(schema.license)
        .where(eq(schema.license.id, licenseId))
        .limit(1)
        .get();
      
      console.log('[Dashboard API] Created new license:', license ? `id=${license.id}, tier=${license.tier}` : 'failed');
    }

    if (!license) {
      return new Response(JSON.stringify({ error: "Failed to create license" }), {
        status: 500,
        headers: { "Content-Type": "application/json" },
      });
    }

    const machines = await db
      .select()
      .from(schema.machine)
      .where(eq(schema.machine.licenseId, license.id))
      .orderBy(desc(schema.machine.lastSeenAt))
      .all();

    const thirtyDaysAgo = new Date();
    thirtyDaysAgo.setDate(thirtyDaysAgo.getDate() - 30);
    const dateStr30 = thirtyDaysAgo.toISOString().split('T')[0];

    const usageStatsResult = await db
      .select({
        totalCommands: sql<number>`COALESCE(SUM(${schema.usageDaily.commandsRun}), 0)`,
        totalPackagesInstalled: sql<number>`COALESCE(SUM(${schema.usageDaily.packagesInstalled}), 0)`,
        totalPackagesSearched: sql<number>`COALESCE(SUM(${schema.usageDaily.packagesSearched}), 0)`,
        totalRuntimesSwitched: sql<number>`COALESCE(SUM(${schema.usageDaily.runtimesSwitched}), 0)`,
        totalSbomGenerated: sql<number>`COALESCE(SUM(${schema.usageDaily.sbomGenerated}), 0)`,
        totalVulnerabilitiesFound: sql<number>`COALESCE(SUM(${schema.usageDaily.vulnerabilitiesFound}), 0)`,
        totalTimeSavedMs: sql<number>`COALESCE(SUM(${schema.usageDaily.timeSavedMs}), 0)`,
      })
      .from(schema.usageDaily)
      .where(
        and(
          eq(schema.usageDaily.licenseId, license.id),
          gte(schema.usageDaily.date, dateStr30)
        )
      )
      .get();

    const usageStats = {
      total_commands: Number(usageStatsResult?.totalCommands || 0),
      total_packages_installed: Number(usageStatsResult?.totalPackagesInstalled || 0),
      total_packages_searched: Number(usageStatsResult?.totalPackagesSearched || 0),
      total_runtimes_switched: Number(usageStatsResult?.totalRuntimesSwitched || 0),
      total_sbom_generated: Number(usageStatsResult?.totalSbomGenerated || 0),
      total_vulnerabilities_found: Number(usageStatsResult?.totalVulnerabilitiesFound || 0),
      total_time_saved_ms: Number(usageStatsResult?.totalTimeSavedMs || 0),
    };

    const fourteenDaysAgo = new Date();
    fourteenDaysAgo.setDate(fourteenDaysAgo.getDate() - 14);
    const dateStr14 = fourteenDaysAgo.toISOString().split('T')[0];

    const dailyUsage = await db
      .select()
      .from(schema.usageDaily)
      .where(
        and(
          eq(schema.usageDaily.licenseId, license.id),
          gte(schema.usageDaily.date, dateStr14)
        )
      )
      .orderBy(desc(schema.usageDaily.date))
      .all();

    const sevenDaysAgo = new Date();
    sevenDaysAgo.setDate(sevenDaysAgo.getDate() - 7);
    const dateStr7 = sevenDaysAgo.toISOString().split('T')[0];

    const lastWeekStats = await db
      .select({
        totalCommands: sql<number>`COALESCE(SUM(${schema.usageDaily.commandsRun}), 0)`,
        totalTimeSaved: sql<number>`COALESCE(SUM(${schema.usageDaily.timeSavedMs}), 0)`,
      })
      .from(schema.usageDaily)
      .where(
        and(
          eq(schema.usageDaily.licenseId, license.id),
          gte(schema.usageDaily.date, dateStr7)
        )
      )
      .get();

    const previousWeekStats = await db
      .select({
        totalCommands: sql<number>`COALESCE(SUM(${schema.usageDaily.commandsRun}), 0)`,
        totalTimeSaved: sql<number>`COALESCE(SUM(${schema.usageDaily.timeSavedMs}), 0)`,
      })
      .from(schema.usageDaily)
      .where(
        and(
          eq(schema.usageDaily.licenseId, license.id),
          gte(schema.usageDaily.date, dateStr14),
          sql`${schema.usageDaily.date} < ${dateStr7}`
        )
      )
      .get();

    const commandsTrend = previousWeekStats?.totalCommands 
      ? ((Number(lastWeekStats?.totalCommands || 0) - Number(previousWeekStats.totalCommands)) / Number(previousWeekStats.totalCommands)) * 100
      : 0;

    const timeSavedTrend = previousWeekStats?.totalTimeSaved 
      ? ((Number(lastWeekStats?.totalTimeSaved || 0) - Number(previousWeekStats.totalTimeSaved)) / Number(previousWeekStats.totalTimeSaved)) * 100
      : 0;

    const userAchievements = await db
      .select({
        id: schema.userAchievement.id,
        achievementId: schema.userAchievement.achievementId,
        progress: schema.userAchievement.progress,
        isUnlocked: schema.userAchievement.isUnlocked,
        unlockedAt: schema.userAchievement.unlockedAt,
        name: schema.achievementDefinition.name,
        description: schema.achievementDefinition.description,
        icon: schema.achievementDefinition.icon,
        category: schema.achievementDefinition.category,
        points: schema.achievementDefinition.points,
      })
      .from(schema.userAchievement)
      .innerJoin(
        schema.achievementDefinition,
        eq(schema.userAchievement.achievementId, schema.achievementDefinition.id)
      )
      .where(eq(schema.userAchievement.userId, userId))
      .all();

    const achievementMap = new Map(
      userAchievements.map((a) => [a.achievementId, a])
    );

    const achievements = ACHIEVEMENTS.map((def) => {
      const userAch = achievementMap.get(def.id);
      const progress = userAch?.progress || checkAchievementProgress(def, usageStats);
      const isUnlocked = userAch?.isUnlocked || progress >= 100;

      return {
        id: def.id,
        achievement_id: def.id,
        name: def.name,
        description: def.description,
        icon: def.icon,
        category: def.category,
        points: def.points,
        progress: Math.round(progress),
        unlocked: isUnlocked,
        unlocked_at: userAch?.unlockedAt?.toISOString() || null,
      };
    });

    const totalUsers = await db
      .select({ count: sql<number>`COUNT(DISTINCT ${schema.license.userId})` })
      .from(schema.license)
      .get();

    const userRank = await db
      .select({ count: sql<number>`COUNT(*)` })
      .from(schema.usageDaily)
      .groupBy(schema.usageDaily.licenseId)
      .having(sql`SUM(${schema.usageDaily.commandsRun}) < ${usageStats.total_commands}`)
      .all();

    const percentile = totalUsers?.count && totalUsers.count > 0
      ? Math.round((userRank.length / Number(totalUsers.count)) * 100)
      : 0;

    console.log('[Dashboard API] Returning tier to frontend:', license.tier);

    const userRecord = await db
      .select({ role: schema.user.role })
      .from(schema.user)
      .where(eq(schema.user.id, userId))
      .limit(1)
      .get();

    console.log('[Dashboard API] User role query - userId:', userId, 'userRecord:', userRecord, 'role:', userRecord?.role);

    const response: TelemetryDashboardResponse = {
      user: {
        id: session.user.id,
        email: session.user.email,
        name: session.user.name,
        role: userRecord?.role || "user",
      },
      license: {
        id: license.id,
        license_key: license.licenseKey,
        tier: license.tier,
        status: license.status,
        max_machines: license.maxMachines,
        expires_at: license.expiresAt?.toISOString() || null,
        features: getTierFeatures(license.tier),
      },
      machines: machines.map((m) => ({
        id: m.id,
        machine_id: m.machineId,
        hostname: m.hostname,
        os: m.os,
        arch: m.arch,
        omg_version: m.omgVersion,
        is_active: m.isActive ? 1 : 0,
        last_seen_at: m.lastSeenAt.toISOString(),
      })),
      usage: {
        ...usageStats,
        commands_trend: Math.round(commandsTrend * 10) / 10,
        time_saved_trend: Math.round(timeSavedTrend * 10) / 10,
      },
      daily: dailyUsage.map((d) => ({
        date: d.date,
        commands_run: d.commandsRun,
        packages_installed: d.packagesInstalled,
        packages_searched: d.packagesSearched,
        time_saved_ms: d.timeSavedMs,
      })),
      achievements,
      global_stats: usageStats.total_commands > 0 ? {
        top_package: "Coming Soon",
        top_runtime: "Coming Soon",
        percentile: Math.max(percentile, 1),
      } : undefined,
    };

    return new Response(JSON.stringify(response), {
      status: 200,
      headers: {
        "Content-Type": "application/json",
        "Cache-Control": "private, no-cache, no-store, must-revalidate",
      },
    });
  } catch (error) {
    console.error("[Telemetry API] Error:", error);
    return new Response(
      JSON.stringify({ 
        error: "Internal server error",
        message: error instanceof Error ? error.message : "Unknown error"
      }),
      {
        status: 500,
        headers: { "Content-Type": "application/json" },
      }
    );
  }
}
