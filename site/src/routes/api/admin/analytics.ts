import { APIEvent } from "@solidjs/start/server";
import { sql, gte, and, eq } from "drizzle-orm";
import * as schema from "~/db/auth-schema";
import { requireAdmin } from "~/lib/admin";

export async function GET(event: APIEvent) {
  try {
    const adminCheck = await requireAdmin(event);
    if (adminCheck instanceof Response) return adminCheck;
    
    const { db } = adminCheck;

    const totalUsers = await db
      .select({ count: sql<number>`COUNT(*)` })
      .from(schema.user)
      .get();

    const totalLicenses = await db
      .select({ count: sql<number>`COUNT(*)` })
      .from(schema.license)
      .get();

    const licensesByTier = await db
      .select({
        tier: schema.license.tier,
        count: sql<number>`COUNT(*)`,
      })
      .from(schema.license)
      .groupBy(schema.license.tier)
      .all();

    const licensesByStatus = await db
      .select({
        status: schema.license.status,
        count: sql<number>`COUNT(*)`,
      })
      .from(schema.license)
      .groupBy(schema.license.status)
      .all();

    const totalMachines = await db
      .select({ count: sql<number>`COUNT(*)` })
      .from(schema.machine)
      .get();

    const activeMachines = await db
      .select({ count: sql<number>`COUNT(*)` })
      .from(schema.machine)
      .where(eq(schema.machine.isActive, true))
      .get();

    const thirtyDaysAgo = new Date();
    thirtyDaysAgo.setDate(thirtyDaysAgo.getDate() - 30);
    const dateStr30 = thirtyDaysAgo.toISOString().split('T')[0];

    const last30DaysStats = await db
      .select({
        totalCommands: sql<number>`COALESCE(SUM(${schema.usageDaily.commandsRun}), 0)`,
        totalPackagesInstalled: sql<number>`COALESCE(SUM(${schema.usageDaily.packagesInstalled}), 0)`,
        totalPackagesSearched: sql<number>`COALESCE(SUM(${schema.usageDaily.packagesSearched}), 0)`,
        totalRuntimesSwitched: sql<number>`COALESCE(SUM(${schema.usageDaily.runtimesSwitched}), 0)`,
        totalTimeSaved: sql<number>`COALESCE(SUM(${schema.usageDaily.timeSavedMs}), 0)`,
      })
      .from(schema.usageDaily)
      .where(gte(schema.usageDaily.date, dateStr30))
      .get();

    const dailyUsageTrend = await db
      .select({
        date: schema.usageDaily.date,
        commands: sql<number>`SUM(${schema.usageDaily.commandsRun})`,
        packages: sql<number>`SUM(${schema.usageDaily.packagesInstalled})`,
      })
      .from(schema.usageDaily)
      .where(gte(schema.usageDaily.date, dateStr30))
      .groupBy(schema.usageDaily.date)
      .orderBy(schema.usageDaily.date)
      .all();

    const sevenDaysAgo = new Date();
    sevenDaysAgo.setDate(sevenDaysAgo.getDate() - 7);
    const dateStr7 = sevenDaysAgo.toISOString().split('T')[0];

    const newUsersLast7Days = await db
      .select({ count: sql<number>`COUNT(*)` })
      .from(schema.user)
      .where(gte(schema.user.createdAt, sevenDaysAgo.getTime()))
      .get();

    const newUsersLast30Days = await db
      .select({ count: sql<number>`COUNT(*)` })
      .from(schema.user)
      .where(gte(schema.user.createdAt, thirtyDaysAgo.getTime()))
      .get();

    return new Response(
      JSON.stringify({
        overview: {
          totalUsers: Number(totalUsers?.count || 0),
          totalLicenses: Number(totalLicenses?.count || 0),
          totalMachines: Number(totalMachines?.count || 0),
          activeMachines: Number(activeMachines?.count || 0),
          newUsersLast7Days: Number(newUsersLast7Days?.count || 0),
          newUsersLast30Days: Number(newUsersLast30Days?.count || 0),
        },
        licenses: {
          byTier: licensesByTier.map(l => ({
            tier: l.tier,
            count: Number(l.count),
          })),
          byStatus: licensesByStatus.map(l => ({
            status: l.status,
            count: Number(l.count),
          })),
        },
        usage: {
          last30Days: {
            totalCommands: Number(last30DaysStats?.totalCommands || 0),
            totalPackagesInstalled: Number(last30DaysStats?.totalPackagesInstalled || 0),
            totalPackagesSearched: Number(last30DaysStats?.totalPackagesSearched || 0),
            totalRuntimesSwitched: Number(last30DaysStats?.totalRuntimesSwitched || 0),
            totalTimeSavedMs: Number(last30DaysStats?.totalTimeSaved || 0),
            totalTimeSavedHours: Math.round(Number(last30DaysStats?.totalTimeSaved || 0) / 1000 / 60 / 60 * 10) / 10,
          },
          dailyTrend: dailyUsageTrend.map(d => ({
            date: d.date,
            commands: Number(d.commands),
            packages: Number(d.packages),
          })),
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
    console.error("[Admin Analytics API] Error:", error);
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
