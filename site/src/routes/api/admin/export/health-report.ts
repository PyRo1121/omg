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
    const format = url.searchParams.get("format") || "csv";
    const includeHistory = url.searchParams.get("includeHistory") === "true";
    const historyDays = parseInt(url.searchParams.get("historyDays") || "30");

    // Get latest health scores for all users with their info
    const healthData = await db
      .select({
        userId: schema.customerHealthHistory.userId,
        email: schema.user.email,
        name: schema.user.name,
        tier: schema.license.tier,
        status: schema.license.status,
        overallScore: schema.customerHealthHistory.overallScore,
        engagementScore: schema.customerHealthHistory.engagementScore,
        activationScore: schema.customerHealthHistory.activationScore,
        growthScore: schema.customerHealthHistory.growthScore,
        riskScore: schema.customerHealthHistory.riskScore,
        lifecycleStage: schema.customerHealthHistory.lifecycleStage,
        churnProbability: schema.customerHealthHistory.churnProbability,
        upgradeProbability: schema.customerHealthHistory.upgradeProbability,
        commandVelocity7d: schema.customerHealthHistory.commandVelocity7d,
        recordedAt: schema.customerHealthHistory.recordedAt,
      })
      .from(schema.customerHealthHistory)
      .innerJoin(schema.user, eq(schema.customerHealthHistory.userId, schema.user.id))
      .leftJoin(schema.license, eq(schema.user.id, schema.license.userId))
      .where(
        sql`${schema.customerHealthHistory.recordedAt} = (
          SELECT MAX(h2.recorded_at)
          FROM customer_health_history h2
          WHERE h2.user_id = ${schema.customerHealthHistory.userId}
        )`
      )
      .orderBy(desc(schema.customerHealthHistory.overallScore))
      .all();

    // Calculate risk bucket
    const addRiskBucket = (score: number): string => {
      if (score >= 80) return "excellent";
      if (score >= 60) return "good";
      if (score >= 40) return "fair";
      if (score >= 20) return "poor";
      return "critical";
    };

    if (format === "json") {
      const exportData = healthData.map(row => ({
        userId: row.userId,
        email: row.email,
        name: row.name,
        tier: row.tier || "free",
        status: row.status || "active",
        healthScores: {
          overall: row.overallScore,
          engagement: row.engagementScore,
          activation: row.activationScore,
          growth: row.growthScore,
          risk: row.riskScore,
        },
        riskBucket: addRiskBucket(row.overallScore),
        lifecycleStage: row.lifecycleStage,
        predictions: {
          churnProbability: row.churnProbability,
          upgradeProbability: row.upgradeProbability,
        },
        activity: {
          commandVelocity7d: row.commandVelocity7d,
        },
        recordedAt: new Date(row.recordedAt).toISOString(),
      }));

      return new Response(
        JSON.stringify({
          exportedAt: new Date().toISOString(),
          recordCount: exportData.length,
          summary: {
            excellent: exportData.filter(d => d.riskBucket === "excellent").length,
            good: exportData.filter(d => d.riskBucket === "good").length,
            fair: exportData.filter(d => d.riskBucket === "fair").length,
            poor: exportData.filter(d => d.riskBucket === "poor").length,
            critical: exportData.filter(d => d.riskBucket === "critical").length,
          },
          data: exportData,
        }),
        {
          status: 200,
          headers: {
            "Content-Type": "application/json",
            "Content-Disposition": `attachment; filename="health-report-${new Date().toISOString().split('T')[0]}.json"`,
          },
        }
      );
    }

    // Generate CSV
    const headers = [
      "User ID",
      "Email",
      "Name",
      "Tier",
      "Status",
      "Overall Score",
      "Engagement Score",
      "Activation Score",
      "Growth Score",
      "Risk Score",
      "Risk Bucket",
      "Lifecycle Stage",
      "Churn Probability",
      "Upgrade Probability",
      "Command Velocity (7d)",
      "Recorded At",
    ];

    const csvRows = [headers.join(",")];

    for (const row of healthData) {
      const values = [
        row.userId,
        escapeCSV(row.email),
        escapeCSV(row.name || ""),
        row.tier || "free",
        row.status || "active",
        row.overallScore,
        row.engagementScore,
        row.activationScore,
        row.growthScore,
        row.riskScore,
        addRiskBucket(row.overallScore),
        row.lifecycleStage,
        row.churnProbability,
        row.upgradeProbability,
        row.commandVelocity7d,
        new Date(row.recordedAt).toISOString(),
      ];
      csvRows.push(values.join(","));
    }

    const csv = csvRows.join("\n");

    return new Response(csv, {
      status: 200,
      headers: {
        "Content-Type": "text/csv; charset=utf-8",
        "Content-Disposition": `attachment; filename="health-report-${new Date().toISOString().split('T')[0]}.csv"`,
      },
    });
  } catch (error) {
    console.error("[Admin Export Health Report] Error:", error);
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
