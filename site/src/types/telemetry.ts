export interface License {
  id: string;
  userId: string;
  licenseKey: string;
  tier: "free" | "team" | "enterprise";
  status: "active" | "suspended" | "expired" | "cancelled";
  maxMachines: number;
  expiresAt: Date | null;
  createdAt: Date;
  updatedAt: Date;
}

export interface Machine {
  id: string;
  licenseId: string;
  machineId: string;
  hostname: string | null;
  os: string | null;
  arch: string | null;
  omgVersion: string | null;
  isActive: boolean;
  firstSeenAt: Date;
  lastSeenAt: Date;
  createdAt: Date;
}

export interface UsageDaily {
  id: string;
  licenseId: string;
  date: string;
  commandsRun: number;
  packagesInstalled: number;
  packagesSearched: number;
  runtimesSwitched: number;
  sbomGenerated: number;
  vulnerabilitiesFound: number;
  timeSavedMs: number;
  createdAt: Date;
  updatedAt: Date;
}

export interface UsageStats {
  totalCommands: number;
  totalPackagesInstalled: number;
  totalPackagesSearched: number;
  totalRuntimesSwitched: number;
  totalSbomGenerated: number;
  totalVulnerabilitiesFound: number;
  totalTimeSavedMs: number;
}

export interface AchievementProgress {
  id: string;
  achievementId: string;
  name: string;
  description: string;
  icon: string;
  category: string;
  points: number;
  progress: number;
  isUnlocked: boolean;
  unlockedAt: Date | null;
}

export interface TelemetryDashboardResponse {
  user: {
    id: string;
    email: string;
    name: string;
    role: string;
  };
  license: {
    id: string;
    license_key: string;
    tier: string;
    status: string;
    max_machines: number;
    expires_at: string | null;
    features: string[];
  };
  usage: {
    total_commands: number;
    total_packages_installed: number;
    total_packages_searched: number;
    total_runtimes_switched: number;
    total_sbom_generated: number;
    total_vulnerabilities_found: number;
    total_time_saved_ms: number;
    commands_trend?: number;
    time_saved_trend?: number;
  };
  daily: Array<{
    date: string;
    commands_run: number;
    packages_installed: number;
    packages_searched: number;
    time_saved_ms: number;
  }>;
  machines: Array<{
    id: string;
    machine_id: string;
    hostname: string | null;
    os: string | null;
    arch: string | null;
    omg_version: string | null;
    is_active: number;
    last_seen_at: string;
  }>;
  achievements: Array<{
    id: string;
    achievement_id: string;
    name: string;
    description: string;
    icon: string;
    category: string;
    points: number;
    progress: number;
    unlocked: boolean;
    unlocked_at: string | null;
  }>;
  global_stats?: {
    top_package: string;
    top_runtime: string;
    percentile: number;
  };
}

export interface CLITelemetryReport {
  license_key: string;
  machine_id: string;
  hostname?: string;
  os?: string;
  arch?: string;
  omg_version?: string;
  commands_run?: number;
  packages_installed?: number;
  packages_searched?: number;
  runtimes_switched?: number;
  sbom_generated?: number;
  vulnerabilities_found?: number;
  time_saved_ms?: number;
}
