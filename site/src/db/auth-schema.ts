import { sql } from "drizzle-orm";
import { sqliteTable, text, integer, index } from "drizzle-orm/sqlite-core";

export const user = sqliteTable("user", {
  id: text("id").primaryKey(),
  name: text("name").notNull(),
  email: text("email").notNull().unique(),
  emailVerified: integer("email_verified", { mode: "boolean" })
    .default(false)
    .notNull(),
  image: text("image"),
  role: text("role", { enum: ["user", "admin"] })
    .notNull()
    .default("user"),
  createdAt: integer("created_at", { mode: "timestamp_ms" })
    .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
    .notNull(),
  updatedAt: integer("updated_at", { mode: "timestamp_ms" })
    .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
    .notNull(),
});

export const session = sqliteTable(
  "session",
  {
    id: text("id").primaryKey(),
    expiresAt: integer("expires_at", { mode: "timestamp_ms" }).notNull(),
    token: text("token").notNull().unique(),
    createdAt: integer("created_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
    updatedAt: integer("updated_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
    ipAddress: text("ip_address"),
    userAgent: text("user_agent"),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
  },
  (table) => [index("session_userId_idx").on(table.userId)],
);

export const account = sqliteTable(
  "account",
  {
    id: text("id").primaryKey(),
    accountId: text("account_id").notNull(),
    providerId: text("provider_id").notNull(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    accessToken: text("access_token"),
    refreshToken: text("refresh_token"),
    idToken: text("id_token"),
    accessTokenExpiresAt: integer("access_token_expires_at", {
      mode: "timestamp_ms",
    }),
    refreshTokenExpiresAt: integer("refresh_token_expires_at", {
      mode: "timestamp_ms",
    }),
    scope: text("scope"),
    password: text("password"),
    createdAt: integer("created_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
    updatedAt: integer("updated_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
  },
  (table) => [index("account_userId_idx").on(table.userId)],
);

export const verification = sqliteTable(
  "verification",
  {
    id: text("id").primaryKey(),
    identifier: text("identifier").notNull(),
    value: text("value").notNull(),
    expiresAt: integer("expires_at", { mode: "timestamp_ms" }).notNull(),
    createdAt: integer("created_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
    updatedAt: integer("updated_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
  },
  (table) => [index("verification_identifier_idx").on(table.identifier)],
);

export const license = sqliteTable(
  "license",
  {
    id: text("id").primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    licenseKey: text("license_key").notNull().unique(),
    tier: text("tier", { enum: ["free", "team", "enterprise"] })
      .notNull()
      .default("free"),
    status: text("status", { enum: ["active", "suspended", "expired", "cancelled"] })
      .notNull()
      .default("active"),
    maxMachines: integer("max_machines").notNull().default(1),
    expiresAt: integer("expires_at", { mode: "timestamp_ms" }),
    createdAt: integer("created_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
    updatedAt: integer("updated_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
  },
  (table) => [
    index("license_userId_idx").on(table.userId),
    index("license_key_idx").on(table.licenseKey),
    index("license_status_idx").on(table.status),
  ],
);

export const machine = sqliteTable(
  "machine",
  {
    id: text("id").primaryKey(),
    licenseId: text("license_id")
      .notNull()
      .references(() => license.id, { onDelete: "cascade" }),
    machineId: text("machine_id").notNull(),
    hostname: text("hostname"),
    os: text("os"),
    arch: text("arch"),
    omgVersion: text("omg_version"),
    isActive: integer("is_active", { mode: "boolean" }).notNull().default(true),
    firstSeenAt: integer("first_seen_at", { mode: "timestamp_ms" }).notNull(),
    lastSeenAt: integer("last_seen_at", { mode: "timestamp_ms" }).notNull(),
    createdAt: integer("created_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
  },
  (table) => [
    index("machine_licenseId_idx").on(table.licenseId),
    index("machine_machineId_idx").on(table.machineId),
    index("machine_isActive_idx").on(table.isActive),
    index("machine_lastSeenAt_idx").on(table.lastSeenAt),
  ],
);

export const usageDaily = sqliteTable(
  "usage_daily",
  {
    id: text("id").primaryKey(),
    licenseId: text("license_id")
      .notNull()
      .references(() => license.id, { onDelete: "cascade" }),
    date: text("date").notNull(),
    commandsRun: integer("commands_run").notNull().default(0),
    packagesInstalled: integer("packages_installed").notNull().default(0),
    packagesSearched: integer("packages_searched").notNull().default(0),
    runtimesSwitched: integer("runtimes_switched").notNull().default(0),
    sbomGenerated: integer("sbom_generated").notNull().default(0),
    vulnerabilitiesFound: integer("vulnerabilities_found").notNull().default(0),
    timeSavedMs: integer("time_saved_ms").notNull().default(0),
    createdAt: integer("created_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
    updatedAt: integer("updated_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
  },
  (table) => [
    index("usage_licenseId_date_idx").on(table.licenseId, table.date),
    index("usage_date_idx").on(table.date),
  ],
);

export const achievementDefinition = sqliteTable(
  "achievement_definition",
  {
    id: text("id").primaryKey(),
    name: text("name").notNull(),
    description: text("description").notNull(),
    icon: text("icon").notNull(),
    category: text("category", { 
      enum: ["milestone", "speed", "explorer", "master", "special"] 
    }).notNull(),
    requirement: text("requirement").notNull(),
    points: integer("points").notNull().default(10),
    isHidden: integer("is_hidden", { mode: "boolean" }).notNull().default(false),
    sortOrder: integer("sort_order").notNull().default(0),
    createdAt: integer("created_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
  },
  (table) => [
    index("achievement_category_idx").on(table.category),
    index("achievement_sortOrder_idx").on(table.sortOrder),
  ],
);

export const userAchievement = sqliteTable(
  "user_achievement",
  {
    id: text("id").primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    achievementId: text("achievement_id")
      .notNull()
      .references(() => achievementDefinition.id, { onDelete: "cascade" }),
    progress: integer("progress").notNull().default(0),
    isUnlocked: integer("is_unlocked", { mode: "boolean" }).notNull().default(false),
    unlockedAt: integer("unlocked_at", { mode: "timestamp_ms" }),
    createdAt: integer("created_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
  },
  (table) => [
    index("userAchievement_userId_idx").on(table.userId),
    index("userAchievement_achievementId_idx").on(table.achievementId),
    index("userAchievement_isUnlocked_idx").on(table.isUnlocked),
  ],
);

// CRM Tables
export const customerNote = sqliteTable(
  "customer_note",
  {
    id: text("id").primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    authorId: text("author_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    content: text("content").notNull(),
    noteType: text("note_type", {
      enum: ["general", "call", "meeting", "support", "escalation"]
    }).notNull().default("general"),
    isPinned: integer("is_pinned", { mode: "boolean" }).notNull().default(false),
    createdAt: integer("created_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
    updatedAt: integer("updated_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
  },
  (table) => [
    index("customerNote_userId_idx").on(table.userId),
    index("customerNote_authorId_idx").on(table.authorId),
    index("customerNote_createdAt_idx").on(table.createdAt),
  ],
);

export const customerTag = sqliteTable(
  "customer_tag",
  {
    id: text("id").primaryKey(),
    name: text("name").notNull().unique(),
    color: text("color").notNull().default("#6366f1"),
    description: text("description"),
    createdAt: integer("created_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
  },
  (table) => [
    index("customerTag_name_idx").on(table.name),
  ],
);

export const customerTagAssignment = sqliteTable(
  "customer_tag_assignment",
  {
    id: text("id").primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    tagId: text("tag_id")
      .notNull()
      .references(() => customerTag.id, { onDelete: "cascade" }),
    assignedBy: text("assigned_by")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    createdAt: integer("created_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
  },
  (table) => [
    index("customerTagAssignment_userId_idx").on(table.userId),
    index("customerTagAssignment_tagId_idx").on(table.tagId),
  ],
);

export const customerHealthHistory = sqliteTable(
  "customer_health_history",
  {
    id: text("id").primaryKey(),
    userId: text("user_id")
      .notNull()
      .references(() => user.id, { onDelete: "cascade" }),
    overallScore: integer("overall_score").notNull(),
    engagementScore: integer("engagement_score").notNull(),
    activationScore: integer("activation_score").notNull(),
    growthScore: integer("growth_score").notNull(),
    riskScore: integer("risk_score").notNull(),
    lifecycleStage: text("lifecycle_stage", {
      enum: ["trial", "active", "power_user", "at_risk", "churned"]
    }).notNull(),
    churnProbability: integer("churn_probability").notNull().default(0),
    upgradeProbability: integer("upgrade_probability").notNull().default(0),
    commandVelocity7d: integer("command_velocity_7d").notNull().default(0),
    recordedAt: integer("recorded_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
  },
  (table) => [
    index("customerHealthHistory_userId_idx").on(table.userId),
    index("customerHealthHistory_recordedAt_idx").on(table.recordedAt),
  ],
);

// Command usage tracking for analytics
export const commandUsage = sqliteTable(
  "command_usage",
  {
    id: text("id").primaryKey(),
    licenseId: text("license_id")
      .notNull()
      .references(() => license.id, { onDelete: "cascade" }),
    command: text("command").notNull(),
    packageName: text("package_name"),
    runtimeName: text("runtime_name"),
    success: integer("success", { mode: "boolean" }).notNull().default(true),
    durationMs: integer("duration_ms"),
    createdAt: integer("created_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
  },
  (table) => [
    index("commandUsage_licenseId_idx").on(table.licenseId),
    index("commandUsage_command_idx").on(table.command),
    index("commandUsage_createdAt_idx").on(table.createdAt),
  ],
);

// Geo tracking for analytics
export const geoUsage = sqliteTable(
  "geo_usage",
  {
    id: text("id").primaryKey(),
    licenseId: text("license_id")
      .notNull()
      .references(() => license.id, { onDelete: "cascade" }),
    countryCode: text("country_code").notNull(),
    region: text("region"),
    city: text("city"),
    timezone: text("timezone"),
    lastSeenAt: integer("last_seen_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
    createdAt: integer("created_at", { mode: "timestamp_ms" })
      .default(sql`(cast(unixepoch('subsecond') * 1000 as integer))`)
      .notNull(),
  },
  (table) => [
    index("geoUsage_licenseId_idx").on(table.licenseId),
    index("geoUsage_countryCode_idx").on(table.countryCode),
  ],
);
