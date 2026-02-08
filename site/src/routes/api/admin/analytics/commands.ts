import { APIEvent } from "@solidjs/start/server";
import { sql, gte, desc, eq } from "drizzle-orm";
import * as schema from "~/db/auth-schema";
import { requireAdmin } from "~/lib/admin";

export async function GET(event: APIEvent) {
  try {
    const adminCheck = await requireAdmin(event);
    if (adminCheck instanceof Response) return adminCheck;

    const { db } = adminCheck;

    const url = new URL(event.request.url);
    const days = parseInt(url.searchParams.get("days") || "30");
    const limit = Math.min(parseInt(url.searchParams.get("limit") || "20"), 100);
    const offset = parseInt(url.searchParams.get("offset") || "0");

    const startDate = new Date();
    startDate.setDate(startDate.getDate() - days);
    const dateStr = startDate.toISOString().split('T')[0];

    // Command distribution from commandUsage table
    const commandDistribution = await db
      .select({
        command: schema.commandUsage.command,
        count: sql<number>`COUNT(*)`,
        successCount: sql<number>`SUM(CASE WHEN ${schema.commandUsage.success} = 1 THEN 1 ELSE 0 END)`,
        failureCount: sql<number>`SUM(CASE WHEN ${schema.commandUsage.success} = 0 THEN 1 ELSE 0 END)`,
        avgDurationMs: sql<number>`AVG(${schema.commandUsage.durationMs})`,
      })
      .from(schema.commandUsage)
      .where(gte(schema.commandUsage.createdAt, startDate.getTime()))
      .groupBy(schema.commandUsage.command)
      .orderBy(desc(sql`COUNT(*)`))
      .limit(limit)
      .offset(offset)
      .all();

    // Popular packages
    const popularPackages = await db
      .select({
        packageName: schema.commandUsage.packageName,
        count: sql<number>`COUNT(*)`,
        uniqueUsers: sql<number>`COUNT(DISTINCT ${schema.commandUsage.licenseId})`,
      })
      .from(schema.commandUsage)
      .where(
        sql`${schema.commandUsage.packageName} IS NOT NULL AND ${schema.commandUsage.createdAt} >= ${startDate.getTime()}`
      )
      .groupBy(schema.commandUsage.packageName)
      .orderBy(desc(sql`COUNT(*)`))
      .limit(limit)
      .all();

    // Command trends over time (daily aggregation)
    const commandTrends = await db
      .select({
        date: sql<string>`date(${schema.commandUsage.createdAt} / 1000, 'unixepoch')`,
        command: schema.commandUsage.command,
        count: sql<number>`COUNT(*)`,
      })
      .from(schema.commandUsage)
      .where(gte(schema.commandUsage.createdAt, startDate.getTime()))
      .groupBy(
        sql`date(${schema.commandUsage.createdAt} / 1000, 'unixepoch')`,
        schema.commandUsage.command
      )
      .orderBy(sql`date(${schema.commandUsage.createdAt} / 1000, 'unixepoch')`)
      .all();

    // Runtime usage
    const runtimeUsage = await db
      .select({
        runtimeName: schema.commandUsage.runtimeName,
        count: sql<number>`COUNT(*)`,
        uniqueUsers: sql<number>`COUNT(DISTINCT ${schema.commandUsage.licenseId})`,
      })
      .from(schema.commandUsage)
      .where(
        sql`${schema.commandUsage.runtimeName} IS NOT NULL AND ${schema.commandUsage.createdAt} >= ${startDate.getTime()}`
      )
      .groupBy(schema.commandUsage.runtimeName)
      .orderBy(desc(sql`COUNT(*)`))
      .limit(limit)
      .all();

    // Overall stats from usageDaily for the period
    const overallStats = await db
      .select({
        totalCommands: sql<number>`COALESCE(SUM(${schema.usageDaily.commandsRun}), 0)`,
        totalPackagesInstalled: sql<number>`COALESCE(SUM(${schema.usageDaily.packagesInstalled}), 0)`,
        totalPackagesSearched: sql<number>`COALESCE(SUM(${schema.usageDaily.packagesSearched}), 0)`,
        totalRuntimesSwitched: sql<number>`COALESCE(SUM(${schema.usageDaily.runtimesSwitched}), 0)`,
        totalSbomGenerated: sql<number>`COALESCE(SUM(${schema.usageDaily.sbomGenerated}), 0)`,
        uniqueLicenses: sql<number>`COUNT(DISTINCT ${schema.usageDaily.licenseId})`,
      })
      .from(schema.usageDaily)
      .where(gte(schema.usageDaily.date, dateStr))
      .get();

    // Daily command totals
    const dailyTotals = await db
      .select({
        date: schema.usageDaily.date,
        commands: sql<number>`SUM(${schema.usageDaily.commandsRun})`,
        packages: sql<number>`SUM(${schema.usageDaily.packagesInstalled})`,
        searches: sql<number>`SUM(${schema.usageDaily.packagesSearched})`,
      })
      .from(schema.usageDaily)
      .where(gte(schema.usageDaily.date, dateStr))
      .groupBy(schema.usageDaily.date)
      .orderBy(schema.usageDaily.date)
      .all();

    // Total count for pagination
    const totalCommands = await db
      .select({ count: sql<number>`COUNT(DISTINCT ${schema.commandUsage.command})` })
      .from(schema.commandUsage)
      .where(gte(schema.commandUsage.createdAt, startDate.getTime()))
      .get();

    return new Response(
      JSON.stringify({
        distribution: commandDistribution.map(c => ({
          command: c.command,
          count: Number(c.count),
          successCount: Number(c.successCount),
          failureCount: Number(c.failureCount),
          successRate: c.count > 0
            ? Math.round((Number(c.successCount) / Number(c.count)) * 100)
            : 0,
          avgDurationMs: Math.round(Number(c.avgDurationMs) || 0),
        })),
        popularPackages: popularPackages.map(p => ({
          packageName: p.packageName,
          count: Number(p.count),
          uniqueUsers: Number(p.uniqueUsers),
        })),
        runtimeUsage: runtimeUsage.map(r => ({
          runtimeName: r.runtimeName,
          count: Number(r.count),
          uniqueUsers: Number(r.uniqueUsers),
        })),
        trends: commandTrends.map(t => ({
          date: t.date,
          command: t.command,
          count: Number(t.count),
        })),
        dailyTotals: dailyTotals.map(d => ({
          date: d.date,
          commands: Number(d.commands),
          packages: Number(d.packages),
          searches: Number(d.searches),
        })),
        summary: {
          totalCommands: Number(overallStats?.totalCommands || 0),
          totalPackagesInstalled: Number(overallStats?.totalPackagesInstalled || 0),
          totalPackagesSearched: Number(overallStats?.totalPackagesSearched || 0),
          totalRuntimesSwitched: Number(overallStats?.totalRuntimesSwitched || 0),
          totalSbomGenerated: Number(overallStats?.totalSbomGenerated || 0),
          uniqueUsers: Number(overallStats?.uniqueLicenses || 0),
          periodDays: days,
        },
        pagination: {
          limit,
          offset,
          total: Number(totalCommands?.count || 0),
        },
      }),
      {
        status: 200,
        headers: {
          "Content-Type": "application/json",
          "Cache-Control": "private, no-cache, no-store, must-revalidate",
        },
      }
    );
  } catch (error) {
    console.error("[Admin Commands Analytics] Error:", error);
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
