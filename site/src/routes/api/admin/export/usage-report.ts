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
    const format = url.searchParams.get("format") || "csv";

    const startDate = new Date();
    startDate.setDate(startDate.getDate() - days);
    const dateStr = startDate.toISOString().split('T')[0];

    // Get usage data with user and license info
    const usageData = await db
      .select({
        date: schema.usageDaily.date,
        userId: schema.license.userId,
        email: schema.user.email,
        name: schema.user.name,
        tier: schema.license.tier,
        status: schema.license.status,
        commandsRun: schema.usageDaily.commandsRun,
        packagesInstalled: schema.usageDaily.packagesInstalled,
        packagesSearched: schema.usageDaily.packagesSearched,
        runtimesSwitched: schema.usageDaily.runtimesSwitched,
        sbomGenerated: schema.usageDaily.sbomGenerated,
        vulnerabilitiesFound: schema.usageDaily.vulnerabilitiesFound,
        timeSavedMs: schema.usageDaily.timeSavedMs,
      })
      .from(schema.usageDaily)
      .innerJoin(schema.license, eq(schema.usageDaily.licenseId, schema.license.id))
      .innerJoin(schema.user, eq(schema.license.userId, schema.user.id))
      .where(gte(schema.usageDaily.date, dateStr))
      .orderBy(desc(schema.usageDaily.date), schema.user.email)
      .all();

    if (format === "json") {
      return new Response(
        JSON.stringify({
          exportedAt: new Date().toISOString(),
          periodStart: dateStr,
          periodEnd: new Date().toISOString().split('T')[0],
          recordCount: usageData.length,
          data: usageData.map(row => ({
            date: row.date,
            userId: row.userId,
            email: row.email,
            name: row.name,
            tier: row.tier,
            status: row.status,
            commandsRun: row.commandsRun,
            packagesInstalled: row.packagesInstalled,
            packagesSearched: row.packagesSearched,
            runtimesSwitched: row.runtimesSwitched,
            sbomGenerated: row.sbomGenerated,
            vulnerabilitiesFound: row.vulnerabilitiesFound,
            timeSavedMs: row.timeSavedMs,
            timeSavedMinutes: Math.round(row.timeSavedMs / 60000 * 10) / 10,
          })),
        }),
        {
          status: 200,
          headers: {
            "Content-Type": "application/json",
            "Content-Disposition": `attachment; filename="usage-report-${dateStr}-to-${new Date().toISOString().split('T')[0]}.json"`,
          },
        }
      );
    }

    // Generate CSV
    const headers = [
      "Date",
      "User ID",
      "Email",
      "Name",
      "Tier",
      "Status",
      "Commands Run",
      "Packages Installed",
      "Packages Searched",
      "Runtimes Switched",
      "SBOM Generated",
      "Vulnerabilities Found",
      "Time Saved (ms)",
      "Time Saved (minutes)",
    ];

    const csvRows = [headers.join(",")];

    for (const row of usageData) {
      const values = [
        row.date,
        row.userId,
        escapeCSV(row.email),
        escapeCSV(row.name || ""),
        row.tier,
        row.status,
        row.commandsRun,
        row.packagesInstalled,
        row.packagesSearched,
        row.runtimesSwitched,
        row.sbomGenerated,
        row.vulnerabilitiesFound,
        row.timeSavedMs,
        Math.round(row.timeSavedMs / 60000 * 10) / 10,
      ];
      csvRows.push(values.join(","));
    }

    const csv = csvRows.join("\n");

    return new Response(csv, {
      status: 200,
      headers: {
        "Content-Type": "text/csv; charset=utf-8",
        "Content-Disposition": `attachment; filename="usage-report-${dateStr}-to-${new Date().toISOString().split('T')[0]}.csv"`,
      },
    });
  } catch (error) {
    console.error("[Admin Export Usage Report] Error:", error);
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

function escapeCSV(value: string): string {
  if (value.includes(",") || value.includes('"') || value.includes("\n")) {
    return `"${value.replace(/"/g, '""')}"`;
  }
  return value;
}
