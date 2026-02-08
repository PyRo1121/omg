import { APIEvent } from "@solidjs/start/server";
import { sql, gte, desc, eq, lte, and } from "drizzle-orm";
import * as schema from "~/db/auth-schema";
import { requireAdmin } from "~/lib/admin";

export async function GET(event: APIEvent) {
  try {
    const adminCheck = await requireAdmin(event);
    if (adminCheck instanceof Response) return adminCheck;

    const { db } = adminCheck;

    const url = new URL(event.request.url);
    const limit = Math.min(parseInt(url.searchParams.get("limit") || "50"), 200);
    const offset = parseInt(url.searchParams.get("offset") || "0");
    const riskThreshold = parseInt(url.searchParams.get("riskThreshold") || "30");

    // Get the latest health scores for all users
    const latestHealthScores = await db
      .select({
        userId: schema.customerHealthHistory.userId,
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
      .where(
        sql`${schema.customerHealthHistory.recordedAt} = (
          SELECT MAX(h2.recorded_at)
          FROM customer_health_history h2
          WHERE h2.user_id = ${schema.customerHealthHistory.userId}
        )`
      )
      .orderBy(desc(schema.customerHealthHistory.overallScore))
      .limit(limit)
      .offset(offset)
      .all();

    // Health score distribution buckets
    const scoreDistribution = await db
      .select({
        bucket: sql<string>`
          CASE
            WHEN ${schema.customerHealthHistory.overallScore} >= 80 THEN 'excellent'
            WHEN ${schema.customerHealthHistory.overallScore} >= 60 THEN 'good'
            WHEN ${schema.customerHealthHistory.overallScore} >= 40 THEN 'fair'
            WHEN ${schema.customerHealthHistory.overallScore} >= 20 THEN 'poor'
            ELSE 'critical'
          END
        `,
        count: sql<number>`COUNT(DISTINCT ${schema.customerHealthHistory.userId})`,
      })
      .from(schema.customerHealthHistory)
      .where(
        sql`${schema.customerHealthHistory.recordedAt} = (
          SELECT MAX(h2.recorded_at)
          FROM customer_health_history h2
          WHERE h2.user_id = ${schema.customerHealthHistory.userId}
        )`
      )
      .groupBy(sql`
        CASE
          WHEN ${schema.customerHealthHistory.overallScore} >= 80 THEN 'excellent'
          WHEN ${schema.customerHealthHistory.overallScore} >= 60 THEN 'good'
          WHEN ${schema.customerHealthHistory.overallScore} >= 40 THEN 'fair'
          WHEN ${schema.customerHealthHistory.overallScore} >= 20 THEN 'poor'
          ELSE 'critical'
        END
      `)
      .all();

    // Lifecycle stage distribution
    const lifecycleDistribution = await db
      .select({
        stage: schema.customerHealthHistory.lifecycleStage,
        count: sql<number>`COUNT(DISTINCT ${schema.customerHealthHistory.userId})`,
        avgScore: sql<number>`AVG(${schema.customerHealthHistory.overallScore})`,
      })
      .from(schema.customerHealthHistory)
      .where(
        sql`${schema.customerHealthHistory.recordedAt} = (
          SELECT MAX(h2.recorded_at)
          FROM customer_health_history h2
          WHERE h2.user_id = ${schema.customerHealthHistory.userId}
        )`
      )
      .groupBy(schema.customerHealthHistory.lifecycleStage)
      .all();

    // At-risk customers (score below threshold or high churn probability)
    const atRiskCustomers = await db
      .select({
        userId: schema.customerHealthHistory.userId,
        email: schema.user.email,
        name: schema.user.name,
        overallScore: schema.customerHealthHistory.overallScore,
        churnProbability: schema.customerHealthHistory.churnProbability,
        lifecycleStage: schema.customerHealthHistory.lifecycleStage,
        commandVelocity7d: schema.customerHealthHistory.commandVelocity7d,
        tier: schema.license.tier,
      })
      .from(schema.customerHealthHistory)
      .innerJoin(schema.user, eq(schema.customerHealthHistory.userId, schema.user.id))
      .leftJoin(schema.license, eq(schema.user.id, schema.license.userId))
      .where(
        and(
          sql`${schema.customerHealthHistory.recordedAt} = (
            SELECT MAX(h2.recorded_at)
            FROM customer_health_history h2
            WHERE h2.user_id = ${schema.customerHealthHistory.userId}
          )`,
          sql`(${schema.customerHealthHistory.overallScore} < ${riskThreshold} OR ${schema.customerHealthHistory.churnProbability} > 70)`
        )
      )
      .orderBy(desc(schema.customerHealthHistory.churnProbability))
      .limit(limit)
      .all();

    // Customers with upgrade potential
    const upgradeOpportunities = await db
      .select({
        userId: schema.customerHealthHistory.userId,
        email: schema.user.email,
        name: schema.user.name,
        overallScore: schema.customerHealthHistory.overallScore,
        upgradeProbability: schema.customerHealthHistory.upgradeProbability,
        lifecycleStage: schema.customerHealthHistory.lifecycleStage,
        tier: schema.license.tier,
      })
      .from(schema.customerHealthHistory)
      .innerJoin(schema.user, eq(schema.customerHealthHistory.userId, schema.user.id))
      .leftJoin(schema.license, eq(schema.user.id, schema.license.userId))
      .where(
        and(
          sql`${schema.customerHealthHistory.recordedAt} = (
            SELECT MAX(h2.recorded_at)
            FROM customer_health_history h2
            WHERE h2.user_id = ${schema.customerHealthHistory.userId}
          )`,
          sql`${schema.customerHealthHistory.upgradeProbability} > 60`,
          sql`${schema.license.tier} != 'enterprise'`
        )
      )
      .orderBy(desc(schema.customerHealthHistory.upgradeProbability))
      .limit(20)
      .all();

    // Average scores across all customers
    const avgScores = await db
      .select({
        avgOverall: sql<number>`AVG(${schema.customerHealthHistory.overallScore})`,
        avgEngagement: sql<number>`AVG(${schema.customerHealthHistory.engagementScore})`,
        avgActivation: sql<number>`AVG(${schema.customerHealthHistory.activationScore})`,
        avgGrowth: sql<number>`AVG(${schema.customerHealthHistory.growthScore})`,
        avgRisk: sql<number>`AVG(${schema.customerHealthHistory.riskScore})`,
        avgChurnProb: sql<number>`AVG(${schema.customerHealthHistory.churnProbability})`,
        totalUsers: sql<number>`COUNT(DISTINCT ${schema.customerHealthHistory.userId})`,
      })
      .from(schema.customerHealthHistory)
      .where(
        sql`${schema.customerHealthHistory.recordedAt} = (
          SELECT MAX(h2.recorded_at)
          FROM customer_health_history h2
          WHERE h2.user_id = ${schema.customerHealthHistory.userId}
        )`
      )
      .get();

    return new Response(
      JSON.stringify({
        healthScores: latestHealthScores.map(h => ({
          userId: h.userId,
          overallScore: h.overallScore,
          engagementScore: h.engagementScore,
          activationScore: h.activationScore,
          growthScore: h.growthScore,
          riskScore: h.riskScore,
          lifecycleStage: h.lifecycleStage,
          churnProbability: h.churnProbability,
          upgradeProbability: h.upgradeProbability,
          commandVelocity7d: h.commandVelocity7d,
          recordedAt: new Date(h.recordedAt).toISOString(),
        })),
        distribution: {
          byScore: scoreDistribution.map(s => ({
            bucket: s.bucket,
            count: Number(s.count),
          })),
          byLifecycle: lifecycleDistribution.map(l => ({
            stage: l.stage,
            count: Number(l.count),
            avgScore: Math.round(Number(l.avgScore) || 0),
          })),
        },
        atRisk: atRiskCustomers.map(c => ({
          userId: c.userId,
          email: c.email,
          name: c.name,
          overallScore: c.overallScore,
          churnProbability: c.churnProbability,
          lifecycleStage: c.lifecycleStage,
          commandVelocity7d: c.commandVelocity7d,
          tier: c.tier || "free",
        })),
        upgradeOpportunities: upgradeOpportunities.map(c => ({
          userId: c.userId,
          email: c.email,
          name: c.name,
          overallScore: c.overallScore,
          upgradeProbability: c.upgradeProbability,
          lifecycleStage: c.lifecycleStage,
          tier: c.tier || "free",
        })),
        summary: {
          avgOverallScore: Math.round(Number(avgScores?.avgOverall) || 0),
          avgEngagementScore: Math.round(Number(avgScores?.avgEngagement) || 0),
          avgActivationScore: Math.round(Number(avgScores?.avgActivation) || 0),
          avgGrowthScore: Math.round(Number(avgScores?.avgGrowth) || 0),
          avgRiskScore: Math.round(Number(avgScores?.avgRisk) || 0),
          avgChurnProbability: Math.round(Number(avgScores?.avgChurnProb) || 0),
          totalTrackedUsers: Number(avgScores?.totalUsers || 0),
          atRiskCount: atRiskCustomers.length,
          upgradeOpportunityCount: upgradeOpportunities.length,
        },
        pagination: {
          limit,
          offset,
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
    console.error("[Admin Health Analytics] Error:", error);
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
