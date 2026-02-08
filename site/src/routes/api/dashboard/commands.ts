import { APIEvent } from "@solidjs/start/server";
import { drizzle } from "drizzle-orm/d1";
import { eq, desc, gte, lte, and, sql } from "drizzle-orm";
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

export interface CommandHistoryItem {
  id: string;
  command: string;
  packageName: string | null;
  runtimeName: string | null;
  success: boolean;
  durationMs: number | null;
  createdAt: string;
  machineHostname: string | null;
}

export interface CommandHistoryResponse {
  commands: CommandHistoryItem[];
  pagination: {
    limit: number;
    offset: number;
    total: number;
    hasMore: boolean;
  };
  filters: {
    availableCommands: string[];
  };
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

    // Parse query parameters
    const url = new URL(event.request.url);
    const limit = Math.min(parseInt(url.searchParams.get("limit") || "50"), 100);
    const offset = parseInt(url.searchParams.get("offset") || "0");
    const commandFilter = url.searchParams.get("command");
    const startDate = url.searchParams.get("startDate");
    const endDate = url.searchParams.get("endDate");
    const successFilter = url.searchParams.get("success");

    // Build query conditions
    const conditions = [eq(schema.commandUsage.licenseId, licenseId)];

    if (commandFilter) {
      conditions.push(eq(schema.commandUsage.command, commandFilter));
    }

    if (startDate) {
      const startTimestamp = new Date(startDate).getTime();
      conditions.push(gte(schema.commandUsage.createdAt, startTimestamp));
    }

    if (endDate) {
      const endTimestamp = new Date(endDate).getTime() + 24 * 60 * 60 * 1000; // Include full day
      conditions.push(lte(schema.commandUsage.createdAt, endTimestamp));
    }

    if (successFilter !== null && successFilter !== undefined) {
      conditions.push(eq(schema.commandUsage.success, successFilter === "true"));
    }

    // Get total count for pagination
    const totalResult = await db
      .select({ count: sql<number>`COUNT(*)` })
      .from(schema.commandUsage)
      .where(and(...conditions))
      .get();

    const total = Number(totalResult?.count || 0);

    // Get commands with machine info
    const commands = await db
      .select({
        id: schema.commandUsage.id,
        command: schema.commandUsage.command,
        packageName: schema.commandUsage.packageName,
        runtimeName: schema.commandUsage.runtimeName,
        success: schema.commandUsage.success,
        durationMs: schema.commandUsage.durationMs,
        createdAt: schema.commandUsage.createdAt,
      })
      .from(schema.commandUsage)
      .where(and(...conditions))
      .orderBy(desc(schema.commandUsage.createdAt))
      .limit(limit)
      .offset(offset)
      .all();

    // Get available command types for filter dropdown
    const availableCommands = await db
      .select({ command: schema.commandUsage.command })
      .from(schema.commandUsage)
      .where(eq(schema.commandUsage.licenseId, licenseId))
      .groupBy(schema.commandUsage.command)
      .all();

    const response: CommandHistoryResponse = {
      commands: commands.map(c => ({
        id: c.id,
        command: c.command,
        packageName: c.packageName,
        runtimeName: c.runtimeName,
        success: c.success,
        durationMs: c.durationMs,
        createdAt: new Date(c.createdAt).toISOString(),
        machineHostname: null, // Could join with machine table if needed
      })),
      pagination: {
        limit,
        offset,
        total,
        hasMore: offset + limit < total,
      },
      filters: {
        availableCommands: availableCommands.map(c => c.command),
      },
    };

    return new Response(JSON.stringify(response), {
      status: 200,
      headers: {
        "Content-Type": "application/json",
        "Cache-Control": "private, no-cache, no-store, must-revalidate",
      },
    });
  } catch (error) {
    console.error("[Command History API] Error:", error);
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
