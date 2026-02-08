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
    const limit = Math.min(parseInt(url.searchParams.get("limit") || "50"), 200);
    const offset = parseInt(url.searchParams.get("offset") || "0");

    const startDate = new Date();
    startDate.setDate(startDate.getDate() - days);

    // Get geographic distribution from geoUsage table
    const countryDistribution = await db
      .select({
        countryCode: schema.geoUsage.countryCode,
        uniqueUsers: sql<number>`COUNT(DISTINCT ${schema.geoUsage.licenseId})`,
        totalSessions: sql<number>`COUNT(*)`,
      })
      .from(schema.geoUsage)
      .where(gte(schema.geoUsage.lastSeenAt, startDate.getTime()))
      .groupBy(schema.geoUsage.countryCode)
      .orderBy(desc(sql`COUNT(DISTINCT ${schema.geoUsage.licenseId})`))
      .limit(limit)
      .offset(offset)
      .all();

    // Get regional distribution (country + region)
    const regionDistribution = await db
      .select({
        countryCode: schema.geoUsage.countryCode,
        region: schema.geoUsage.region,
        uniqueUsers: sql<number>`COUNT(DISTINCT ${schema.geoUsage.licenseId})`,
      })
      .from(schema.geoUsage)
      .where(
        sql`${schema.geoUsage.region} IS NOT NULL AND ${schema.geoUsage.lastSeenAt} >= ${startDate.getTime()}`
      )
      .groupBy(schema.geoUsage.countryCode, schema.geoUsage.region)
      .orderBy(desc(sql`COUNT(DISTINCT ${schema.geoUsage.licenseId})`))
      .limit(limit)
      .all();

    // Get timezone distribution
    const timezoneDistribution = await db
      .select({
        timezone: schema.geoUsage.timezone,
        uniqueUsers: sql<number>`COUNT(DISTINCT ${schema.geoUsage.licenseId})`,
      })
      .from(schema.geoUsage)
      .where(
        sql`${schema.geoUsage.timezone} IS NOT NULL AND ${schema.geoUsage.lastSeenAt} >= ${startDate.getTime()}`
      )
      .groupBy(schema.geoUsage.timezone)
      .orderBy(desc(sql`COUNT(DISTINCT ${schema.geoUsage.licenseId})`))
      .limit(limit)
      .all();

    // Get city-level data for top cities
    const cityDistribution = await db
      .select({
        countryCode: schema.geoUsage.countryCode,
        city: schema.geoUsage.city,
        uniqueUsers: sql<number>`COUNT(DISTINCT ${schema.geoUsage.licenseId})`,
      })
      .from(schema.geoUsage)
      .where(
        sql`${schema.geoUsage.city} IS NOT NULL AND ${schema.geoUsage.lastSeenAt} >= ${startDate.getTime()}`
      )
      .groupBy(schema.geoUsage.countryCode, schema.geoUsage.city)
      .orderBy(desc(sql`COUNT(DISTINCT ${schema.geoUsage.licenseId})`))
      .limit(20)
      .all();

    // Calculate summary statistics
    const totalStats = await db
      .select({
        totalCountries: sql<number>`COUNT(DISTINCT ${schema.geoUsage.countryCode})`,
        totalUsers: sql<number>`COUNT(DISTINCT ${schema.geoUsage.licenseId})`,
        totalRegions: sql<number>`COUNT(DISTINCT ${schema.geoUsage.countryCode} || '-' || COALESCE(${schema.geoUsage.region}, ''))`,
      })
      .from(schema.geoUsage)
      .where(gte(schema.geoUsage.lastSeenAt, startDate.getTime()))
      .get();

    // OS distribution from machine data (as a proxy for geo+platform)
    const osDistribution = await db
      .select({
        os: schema.machine.os,
        count: sql<number>`COUNT(*)`,
      })
      .from(schema.machine)
      .where(gte(schema.machine.lastSeenAt, startDate.getTime()))
      .groupBy(schema.machine.os)
      .orderBy(desc(sql`COUNT(*)`))
      .all();

    // Calculate percentages for countries
    const totalUsers = Number(totalStats?.totalUsers || 1);
    const countriesWithPercentage = countryDistribution.map(c => ({
      countryCode: c.countryCode,
      uniqueUsers: Number(c.uniqueUsers),
      totalSessions: Number(c.totalSessions),
      percentage: Math.round((Number(c.uniqueUsers) / totalUsers) * 100 * 10) / 10,
    }));

    return new Response(
      JSON.stringify({
        countries: countriesWithPercentage,
        regions: regionDistribution.map(r => ({
          countryCode: r.countryCode,
          region: r.region,
          uniqueUsers: Number(r.uniqueUsers),
        })),
        cities: cityDistribution.map(c => ({
          countryCode: c.countryCode,
          city: c.city,
          uniqueUsers: Number(c.uniqueUsers),
        })),
        timezones: timezoneDistribution.map(t => ({
          timezone: t.timezone,
          uniqueUsers: Number(t.uniqueUsers),
        })),
        platforms: osDistribution.map(o => ({
          os: o.os || "unknown",
          count: Number(o.count),
        })),
        summary: {
          totalCountries: Number(totalStats?.totalCountries || 0),
          totalUsers: Number(totalStats?.totalUsers || 0),
          totalRegions: Number(totalStats?.totalRegions || 0),
          periodDays: days,
        },
        pagination: {
          limit,
          offset,
          total: Number(totalStats?.totalCountries || 0),
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
    console.error("[Admin Geo Analytics] Error:", error);
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
