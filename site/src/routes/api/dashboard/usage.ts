import { APIEvent } from "@solidjs/start/server";
import { drizzle } from "drizzle-orm/d1";
import { eq, desc, gte, sql, and, count } from "drizzle-orm";
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

export interface UserUsageResponse {
  totalCommandsThisMonth: number;
  commandsThisWeek: number;
  commandsTrend: number;
  activeMachinesCount: number;
  totalMachinesCount: number;
  commandDistribution: Array<{
    command: string;
    count: number;
    percentage: number;
  }>;
  featureAdoption: Array<{
    feature: string;
    adopted: boolean;
    usageCount: number;
    lastUsed: string | null;
  }>;
  dailyUsage: Array<{
    date: string;
    commands: number;
    timeSavedMs: number;
  }>;
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

    // Get user's license
    const license = await db
      .select()
      .from(schema.license)
      .where(eq(schema.license.userId, userId))
      .limit(1)
      .get();

    if (!license) {
      return new Response(JSON.stringify({ error: "No license found" }), {
        status: 404,
        headers: { "Content-Type": "application/json" },
      });
    }

    const licenseId = license.id;

    // Calculate date ranges
    const now = new Date();
    const startOfMonth = new Date(now.getFullYear(), now.getMonth(), 1);
    const startOfMonthStr = startOfMonth.toISOString().split('T')[0];

    const sevenDaysAgo = new Date();
    sevenDaysAgo.setDate(sevenDaysAgo.getDate() - 7);
    const weekAgoStr = sevenDaysAgo.toISOString().split('T')[0];

    const fourteenDaysAgo = new Date();
    fourteenDaysAgo.setDate(fourteenDaysAgo.getDate() - 14);
    const twoWeeksAgoStr = fourteenDaysAgo.toISOString().split('T')[0];

    const thirtyDaysAgo = new Date();
    thirtyDaysAgo.setDate(thirtyDaysAgo.getDate() - 30);
    const monthAgoStr = thirtyDaysAgo.toISOString().split('T')[0];

    // Total commands this month from usageDaily
    const monthlyStats = await db
      .select({
        totalCommands: sql<number>`COALESCE(SUM(${schema.usageDaily.commandsRun}), 0)`,
      })
      .from(schema.usageDaily)
      .where(
        and(
          eq(schema.usageDaily.licenseId, licenseId),
          gte(schema.usageDaily.date, startOfMonthStr)
        )
      )
      .get();

    // Commands this week and last week for trend
    const thisWeekStats = await db
      .select({
        totalCommands: sql<number>`COALESCE(SUM(${schema.usageDaily.commandsRun}), 0)`,
      })
      .from(schema.usageDaily)
      .where(
        and(
          eq(schema.usageDaily.licenseId, licenseId),
          gte(schema.usageDaily.date, weekAgoStr)
        )
      )
      .get();

    const lastWeekStats = await db
      .select({
        totalCommands: sql<number>`COALESCE(SUM(${schema.usageDaily.commandsRun}), 0)`,
      })
      .from(schema.usageDaily)
      .where(
        and(
          eq(schema.usageDaily.licenseId, licenseId),
          gte(schema.usageDaily.date, twoWeeksAgoStr),
          sql`${schema.usageDaily.date} < ${weekAgoStr}`
        )
      )
      .get();

    const commandsTrend = lastWeekStats?.totalCommands && Number(lastWeekStats.totalCommands) > 0
      ? ((Number(thisWeekStats?.totalCommands || 0) - Number(lastWeekStats.totalCommands)) / Number(lastWeekStats.totalCommands)) * 100
      : 0;

    // Active machines count
    const oneDayAgo = new Date(Date.now() - 24 * 60 * 60 * 1000);
    const machines = await db
      .select()
      .from(schema.machine)
      .where(eq(schema.machine.licenseId, licenseId))
      .all();

    const activeMachines = machines.filter(m =>
      m.isActive && m.lastSeenAt.getTime() > oneDayAgo.getTime()
    );

    // Command distribution from commandUsage
    const commandDistribution = await db
      .select({
        command: schema.commandUsage.command,
        count: sql<number>`COUNT(*)`,
      })
      .from(schema.commandUsage)
      .where(
        and(
          eq(schema.commandUsage.licenseId, licenseId),
          gte(schema.commandUsage.createdAt, startOfMonth.getTime())
        )
      )
      .groupBy(schema.commandUsage.command)
      .orderBy(desc(sql`COUNT(*)`))
      .all();

    const totalCommands = commandDistribution.reduce((sum, c) => sum + Number(c.count), 0);

    // Feature adoption - check various features
    const featureChecks = [
      { feature: 'aur', check: 'aur' },
      { feature: 'daemon', check: 'daemon' },
      { feature: 'sbom', check: 'sbom' },
      { feature: 'fleet', check: 'fleet' },
      { feature: 'runtimes', check: 'use' },
      { feature: 'audit', check: 'audit' },
    ];

    const featureAdoption = await Promise.all(
      featureChecks.map(async ({ feature, check }) => {
        const usage = await db
          .select({
            count: sql<number>`COUNT(*)`,
            lastUsed: sql<number>`MAX(${schema.commandUsage.createdAt})`,
          })
          .from(schema.commandUsage)
          .where(
            and(
              eq(schema.commandUsage.licenseId, licenseId),
              sql`${schema.commandUsage.command} LIKE '%${check}%' OR ${schema.commandUsage.packageName} LIKE '%${check}%'`
            )
          )
          .get();

        return {
          feature,
          adopted: Number(usage?.count || 0) > 0,
          usageCount: Number(usage?.count || 0),
          lastUsed: usage?.lastUsed ? new Date(Number(usage.lastUsed)).toISOString() : null,
        };
      })
    );

    // Daily usage for the last 30 days
    const dailyUsage = await db
      .select({
        date: schema.usageDaily.date,
        commands: schema.usageDaily.commandsRun,
        timeSavedMs: schema.usageDaily.timeSavedMs,
      })
      .from(schema.usageDaily)
      .where(
        and(
          eq(schema.usageDaily.licenseId, licenseId),
          gte(schema.usageDaily.date, monthAgoStr)
        )
      )
      .orderBy(schema.usageDaily.date)
      .all();

    const response: UserUsageResponse = {
      totalCommandsThisMonth: Number(monthlyStats?.totalCommands || 0),
      commandsThisWeek: Number(thisWeekStats?.totalCommands || 0),
      commandsTrend: Math.round(commandsTrend * 10) / 10,
      activeMachinesCount: activeMachines.length,
      totalMachinesCount: machines.length,
      commandDistribution: commandDistribution.map(c => ({
        command: c.command,
        count: Number(c.count),
        percentage: totalCommands > 0 ? Math.round((Number(c.count) / totalCommands) * 100) : 0,
      })),
      featureAdoption,
      dailyUsage: dailyUsage.map(d => ({
        date: d.date,
        commands: d.commands,
        timeSavedMs: d.timeSavedMs,
      })),
    };

    return new Response(JSON.stringify(response), {
      status: 200,
      headers: {
        "Content-Type": "application/json",
        "Cache-Control": "private, no-cache, no-store, must-revalidate",
      },
    });
  } catch (error) {
    console.error("[User Usage API] Error:", error);
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
