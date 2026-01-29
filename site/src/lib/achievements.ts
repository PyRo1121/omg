export interface AchievementDefinition {
  id: string;
  name: string;
  description: string;
  icon: string;
  category: "milestone" | "speed" | "explorer" | "master" | "special";
  requirement: string;
  points: number;
  isHidden: boolean;
  sortOrder: number;
}

export const ACHIEVEMENTS: AchievementDefinition[] = [
  {
    id: "first_command",
    name: "First Steps",
    description: "Run your first OMG command",
    icon: "Rocket",
    category: "milestone",
    requirement: "commands_run >= 1",
    points: 5,
    isHidden: false,
    sortOrder: 1,
  },
  {
    id: "getting_started",
    name: "Getting Started",
    description: "Run 10 commands",
    icon: "Terminal",
    category: "milestone",
    requirement: "commands_run >= 10",
    points: 10,
    isHidden: false,
    sortOrder: 2,
  },
  {
    id: "power_user",
    name: "Power User",
    description: "Run 100 commands",
    icon: "Zap",
    category: "milestone",
    requirement: "commands_run >= 100",
    points: 25,
    isHidden: false,
    sortOrder: 3,
  },
  {
    id: "command_master",
    name: "Command Master",
    description: "Run 1,000 commands",
    icon: "Crown",
    category: "master",
    requirement: "commands_run >= 1000",
    points: 50,
    isHidden: false,
    sortOrder: 4,
  },
  {
    id: "cli_legend",
    name: "CLI Legend",
    description: "Run 10,000 commands",
    icon: "Trophy",
    category: "master",
    requirement: "commands_run >= 10000",
    points: 100,
    isHidden: false,
    sortOrder: 5,
  },
  {
    id: "package_explorer",
    name: "Package Explorer",
    description: "Search for 50 packages",
    icon: "Search",
    category: "explorer",
    requirement: "packages_searched >= 50",
    points: 15,
    isHidden: false,
    sortOrder: 10,
  },
  {
    id: "package_hunter",
    name: "Package Hunter",
    description: "Search for 500 packages",
    icon: "Target",
    category: "explorer",
    requirement: "packages_searched >= 500",
    points: 30,
    isHidden: false,
    sortOrder: 11,
  },
  {
    id: "package_master",
    name: "Package Master",
    description: "Install 100 packages",
    icon: "Package",
    category: "master",
    requirement: "packages_installed >= 100",
    points: 40,
    isHidden: false,
    sortOrder: 12,
  },
  {
    id: "dependency_manager",
    name: "Dependency Manager",
    description: "Install 500 packages",
    icon: "Box",
    category: "master",
    requirement: "packages_installed >= 500",
    points: 75,
    isHidden: false,
    sortOrder: 13,
  },
  {
    id: "version_switcher",
    name: "Version Switcher",
    description: "Switch between 10 runtime versions",
    icon: "GitBranch",
    category: "explorer",
    requirement: "runtimes_switched >= 10",
    points: 20,
    isHidden: false,
    sortOrder: 20,
  },
  {
    id: "runtime_master",
    name: "Runtime Master",
    description: "Switch between 100 runtime versions",
    icon: "Code",
    category: "master",
    requirement: "runtimes_switched >= 100",
    points: 50,
    isHidden: false,
    sortOrder: 21,
  },
  {
    id: "security_conscious",
    name: "Security Conscious",
    description: "Generate your first SBOM",
    icon: "Shield",
    category: "milestone",
    requirement: "sbom_generated >= 1",
    points: 15,
    isHidden: false,
    sortOrder: 30,
  },
  {
    id: "security_expert",
    name: "Security Expert",
    description: "Generate 50 SBOMs",
    icon: "ShieldCheck",
    category: "master",
    requirement: "sbom_generated >= 50",
    points: 40,
    isHidden: false,
    sortOrder: 31,
  },
  {
    id: "vulnerability_hunter",
    name: "Vulnerability Hunter",
    description: "Find 10 vulnerabilities",
    icon: "Bug",
    category: "special",
    requirement: "vulnerabilities_found >= 10",
    points: 25,
    isHidden: false,
    sortOrder: 32,
  },
  {
    id: "speed_demon",
    name: "Speed Demon",
    description: "Save 1 hour of time with OMG",
    icon: "Zap",
    category: "speed",
    requirement: "time_saved_ms >= 3600000",
    points: 30,
    isHidden: false,
    sortOrder: 40,
  },
  {
    id: "time_saver",
    name: "Time Saver",
    description: "Save 10 hours of time with OMG",
    icon: "Clock",
    category: "speed",
    requirement: "time_saved_ms >= 36000000",
    points: 60,
    isHidden: false,
    sortOrder: 41,
  },
  {
    id: "productivity_king",
    name: "Productivity King",
    description: "Save 100 hours of time with OMG",
    icon: "Crown",
    category: "master",
    requirement: "time_saved_ms >= 360000000",
    points: 150,
    isHidden: false,
    sortOrder: 42,
  },
  {
    id: "early_adopter",
    name: "Early Adopter",
    description: "Join OMG in its first year",
    icon: "Star",
    category: "special",
    requirement: "created_before:2027-01-01",
    points: 50,
    isHidden: false,
    sortOrder: 100,
  },
  {
    id: "daily_driver",
    name: "Daily Driver",
    description: "Use OMG 30 days in a row",
    icon: "Calendar",
    category: "special",
    requirement: "streak >= 30",
    points: 40,
    isHidden: false,
    sortOrder: 101,
  },
  {
    id: "night_owl",
    name: "Night Owl",
    description: "Run 50 commands after midnight",
    icon: "Moon",
    category: "special",
    requirement: "commands_after_midnight >= 50",
    points: 20,
    isHidden: true,
    sortOrder: 200,
  },
  {
    id: "weekend_warrior",
    name: "Weekend Warrior",
    description: "Run 100 commands on weekends",
    icon: "Coffee",
    category: "special",
    requirement: "weekend_commands >= 100",
    points: 20,
    isHidden: true,
    sortOrder: 201,
  },
  {
    id: "perfectionist",
    name: "Perfectionist",
    description: "Complete all non-hidden achievements",
    icon: "Gem",
    category: "special",
    requirement: "all_achievements_unlocked",
    points: 200,
    isHidden: true,
    sortOrder: 999,
  },
];

export function getIconByName(iconName: string) {
  const iconMap: Record<string, string> = {
    Rocket: "Rocket",
    Terminal: "Terminal",
    Zap: "Zap",
    Crown: "Crown",
    Trophy: "Trophy",
    Search: "Search",
    Target: "Target",
    Package: "Package",
    Box: "Box",
    GitBranch: "GitBranch",
    Code: "Code",
    Shield: "Shield",
    ShieldCheck: "ShieldCheck",
    Bug: "Bug",
    Clock: "Clock",
    Star: "Star",
    Calendar: "Calendar",
    Moon: "Moon",
    Coffee: "Coffee",
    Gem: "Gem",
  };
  return iconMap[iconName] || "Target";
}

export function checkAchievementProgress(
  achievement: AchievementDefinition,
  stats: {
    commands_run: number;
    packages_searched: number;
    packages_installed: number;
    runtimes_switched: number;
    sbom_generated: number;
    vulnerabilities_found: number;
    time_saved_ms: number;
  },
): number {
  const req = achievement.requirement;

  if (req.startsWith("commands_run >= ")) {
    const threshold = parseInt(req.split(">=")[1].trim());
    return Math.min(100, (stats.commands_run / threshold) * 100);
  }

  if (req.startsWith("packages_searched >= ")) {
    const threshold = parseInt(req.split(">=")[1].trim());
    return Math.min(100, (stats.packages_searched / threshold) * 100);
  }

  if (req.startsWith("packages_installed >= ")) {
    const threshold = parseInt(req.split(">=")[1].trim());
    return Math.min(100, (stats.packages_installed / threshold) * 100);
  }

  if (req.startsWith("runtimes_switched >= ")) {
    const threshold = parseInt(req.split(">=")[1].trim());
    return Math.min(100, (stats.runtimes_switched / threshold) * 100);
  }

  if (req.startsWith("sbom_generated >= ")) {
    const threshold = parseInt(req.split(">=")[1].trim());
    return Math.min(100, (stats.sbom_generated / threshold) * 100);
  }

  if (req.startsWith("vulnerabilities_found >= ")) {
    const threshold = parseInt(req.split(">=")[1].trim());
    return Math.min(100, (stats.vulnerabilities_found / threshold) * 100);
  }

  if (req.startsWith("time_saved_ms >= ")) {
    const threshold = parseInt(req.split(">=")[1].trim());
    return Math.min(100, (stats.time_saved_ms / threshold) * 100);
  }

  return 0;
}
