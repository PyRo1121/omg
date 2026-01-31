import { APIEvent } from "@solidjs/start/server";
import { sql, desc, eq } from "drizzle-orm";
import * as schema from "~/db/auth-schema";
import { requireAdmin } from "~/lib/admin";

export async function GET(event: APIEvent) {
  try {
    const adminCheck = await requireAdmin(event);
    if (adminCheck instanceof Response) return adminCheck;
    
    const { db } = adminCheck;

    const url = new URL(event.request.url);
    const page = parseInt(url.searchParams.get("page") || "1");
    const limit = parseInt(url.searchParams.get("limit") || "20");
    const search = url.searchParams.get("search") || "";
    const offset = (page - 1) * limit;

    let query = db
      .select({
        id: schema.user.id,
        name: schema.user.name,
        email: schema.user.email,
        role: schema.user.role,
        emailVerified: schema.user.emailVerified,
        createdAt: schema.user.createdAt,
        updatedAt: schema.user.updatedAt,
      })
      .from(schema.user)
      .orderBy(desc(schema.user.createdAt))
      .limit(limit)
      .offset(offset);

    const users = await query.all();

    const usersWithLicenses = await Promise.all(
      users.map(async (user) => {
        const license = await db
          .select()
          .from(schema.license)
          .where(eq(schema.license.userId, user.id))
          .limit(1)
          .get();

        const machineCount = await db
          .select({ count: sql<number>`COUNT(*)` })
          .from(schema.machine)
          .where(eq(schema.machine.licenseId, license?.id || ""))
          .get();

        const usageStats = await db
          .select({
            totalCommands: sql<number>`COALESCE(SUM(${schema.usageDaily.commandsRun}), 0)`,
            totalPackages: sql<number>`COALESCE(SUM(${schema.usageDaily.packagesInstalled}), 0)`,
          })
          .from(schema.usageDaily)
          .where(eq(schema.usageDaily.licenseId, license?.id || ""))
          .get();

        return {
          ...user,
          license: license ? {
            id: license.id,
            key: license.licenseKey,
            tier: license.tier,
            status: license.status,
            maxMachines: license.maxMachines,
            expiresAt: license.expiresAt?.toISOString() || null,
          } : null,
          machines: machineCount?.count || 0,
          totalCommands: Number(usageStats?.totalCommands || 0),
          totalPackages: Number(usageStats?.totalPackages || 0),
        };
      })
    );

    const totalCount = await db
      .select({ count: sql<number>`COUNT(*)` })
      .from(schema.user)
      .get();

    return new Response(
      JSON.stringify({
        users: usersWithLicenses,
        pagination: {
          page,
          limit,
          total: Number(totalCount?.count || 0),
          pages: Math.ceil(Number(totalCount?.count || 0) / limit),
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
    console.error("[Admin Users API] Error:", error);
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
