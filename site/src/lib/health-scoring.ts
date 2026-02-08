/**
 * Customer Health Scoring Algorithm for OMG Package Manager
 *
 * Health score (0-100) is composed of weighted sub-scores:
 * - Engagement Score (40% weight)
 * - Adoption Score (30% weight)
 * - Satisfaction Score (20% weight)
 * - Support Health (10% weight)
 */

// ============================================================================
// Type Definitions
// ============================================================================

export type ChurnRisk = 'low' | 'medium' | 'high' | 'critical';

export type FeatureName = 'daemon' | 'parallel' | 'sbom' | 'fleet' | 'aur' | 'runtimes' | 'audit' | 'pgp' | 'slsa' | 'cache';

export type VelocityTrend = 'growing' | 'stable' | 'declining';

export interface UsageDailyRecord {
  date: string;
  commandsRun: number;
  packagesInstalled: number;
  packagesSearched: number;
  runtimesSwitched: number;
  sbomGenerated: number;
  vulnerabilitiesFound: number;
  timeSavedMs: number;
}

export interface FeatureUsageRecord {
  featureName: FeatureName;
  firstUsed: number;
  lastUsed: number;
  usageCount: number;
}

export interface CommandEventRecord {
  command: string;
  durationMs: number;
  success: boolean;
  timestamp: number;
}

export interface SessionRecord {
  sessionStart: number;
  sessionEnd: number | null;
  commandsCount: number;
  activeMinutes: number;
}

export interface SupportTicket {
  id: string;
  status: 'open' | 'pending' | 'resolved' | 'closed';
  priority: 'low' | 'medium' | 'high' | 'urgent';
  satisfactionRating?: number; // 1-5
  createdAt: number;
  resolvedAt?: number;
}

export interface HealthDataInput {
  // User/License metadata
  licenseId: string;
  tier: 'free' | 'team' | 'enterprise';
  createdAt: number;
  currentOmgVersion: string;
  latestOmgVersion: string;

  // Usage data (last 30 days)
  usageDaily: UsageDailyRecord[];

  // Feature adoption
  featureUsage: FeatureUsageRecord[];

  // Command events (for error rate calculation)
  commandEvents: CommandEventRecord[];

  // Session data
  sessions: SessionRecord[];

  // Achievement progress
  totalAchievements: number;
  unlockedAchievements: number;

  // Support data (optional)
  supportTickets?: SupportTicket[];
  npsScore?: number; // -100 to 100

  // Industry benchmark for time saved (ms per day)
  industryAvgTimeSavedMs?: number;
}

export interface EngagementScore {
  total: number;
  activeDaysScore: number;
  commandsTrendScore: number;
  sessionDurationScore: number;
}

export interface AdoptionScore {
  total: number;
  featureBreadthScore: number;
  advancedFeatureScore: number;
  versionCurrencyScore: number;
}

export interface SatisfactionScore {
  total: number;
  errorRateScore: number;
  timeSavedScore: number;
  achievementProgressScore: number;
}

export interface SupportHealthScore {
  total: number;
  ticketVolumeScore: number;
  resolutionSatisfactionScore: number;
  npsScore: number;
}

export interface HealthScoreResult {
  overallScore: number;
  engagement: EngagementScore;
  adoption: AdoptionScore;
  satisfaction: SatisfactionScore;
  supportHealth: SupportHealthScore;
  churnRisk: ChurnRisk;
  calculatedAt: number;
}

export interface HealthTrends {
  scoreHistory: Array<{ date: string; score: number }>;
  engagementTrend: VelocityTrend;
  commandVelocity7d: number;
  commandVelocity30d: number;
  daysActive30d: number;
  daysSinceLastActivity: number;
}

export interface InterventionSuggestion {
  priority: 'low' | 'medium' | 'high' | 'urgent';
  category: 'engagement' | 'adoption' | 'satisfaction' | 'support' | 'retention';
  title: string;
  description: string;
  suggestedAction: string;
  expectedImpact: string;
}

export interface ChurnPrediction {
  probability: number; // 0.0-1.0
  confidence: number;  // 0.0-1.0
  riskFactors: string[];
  protectiveFactors: string[];
}

// ============================================================================
// Constants
// ============================================================================

const WEIGHTS = {
  engagement: 0.40,
  adoption: 0.30,
  satisfaction: 0.20,
  supportHealth: 0.10,
} as const;

const ADVANCED_FEATURES: FeatureName[] = ['aur', 'sbom', 'fleet', 'slsa', 'audit'];

const ALL_FEATURES: FeatureName[] = ['daemon', 'parallel', 'sbom', 'fleet', 'aur', 'runtimes', 'audit', 'pgp', 'slsa', 'cache'];

// Industry average: 30 minutes saved per day for package management
const DEFAULT_INDUSTRY_AVG_TIME_SAVED_MS = 30 * 60 * 1000;

// ============================================================================
// Engagement Score Calculation (40% weight)
// ============================================================================

function calculateActiveDaysScore(usageDaily: UsageDailyRecord[]): number {
  // Max 30 points if all 30 days active
  const activeDays = usageDaily.filter(d => d.commandsRun > 0).length;
  return Math.min(30, activeDays);
}

function calculateCommandsTrendScore(usageDaily: UsageDailyRecord[]): number {
  // 10 points if increasing, 5 if stable, 0 if declining
  if (usageDaily.length < 7) return 5; // Default to stable if insufficient data

  const sortedDays = [...usageDaily].sort((a, b) => a.date.localeCompare(b.date));

  // Split into first half and second half
  const midpoint = Math.floor(sortedDays.length / 2);
  const firstHalf = sortedDays.slice(0, midpoint);
  const secondHalf = sortedDays.slice(midpoint);

  const firstHalfAvg = firstHalf.reduce((sum, d) => sum + d.commandsRun, 0) / firstHalf.length;
  const secondHalfAvg = secondHalf.reduce((sum, d) => sum + d.commandsRun, 0) / secondHalf.length;

  if (secondHalfAvg > firstHalfAvg * 1.1) return 10; // Growing (>10% increase)
  if (secondHalfAvg < firstHalfAvg * 0.9) return 0;  // Declining (>10% decrease)
  return 5; // Stable
}

function calculateSessionDurationScore(sessions: SessionRecord[]): number {
  // Max 10 points based on average session duration
  if (sessions.length === 0) return 0;

  const avgMinutes = sessions.reduce((sum, s) => sum + s.activeMinutes, 0) / sessions.length;

  // 10+ minutes average = 10 points, scales linearly
  return Math.min(10, avgMinutes);
}

function calculateEngagementScore(data: HealthDataInput): EngagementScore {
  const activeDaysScore = calculateActiveDaysScore(data.usageDaily);
  const commandsTrendScore = calculateCommandsTrendScore(data.usageDaily);
  const sessionDurationScore = calculateSessionDurationScore(data.sessions);

  // Normalize to 0-100
  const rawTotal = activeDaysScore + commandsTrendScore + sessionDurationScore;
  const maxPossible = 30 + 10 + 10; // 50
  const total = Math.round((rawTotal / maxPossible) * 100);

  return {
    total,
    activeDaysScore,
    commandsTrendScore,
    sessionDurationScore,
  };
}

// ============================================================================
// Adoption Score Calculation (30% weight)
// ============================================================================

function calculateFeatureBreadthScore(featureUsage: FeatureUsageRecord[]): number {
  // Percentage of features used (out of all trackable features)
  const uniqueFeatures = new Set(featureUsage.map(f => f.featureName));
  const breadthPercent = (uniqueFeatures.size / ALL_FEATURES.length) * 100;
  return Math.round(breadthPercent * 0.4); // Max 40 points for full adoption
}

function calculateAdvancedFeatureScore(featureUsage: FeatureUsageRecord[]): number {
  // Points for using advanced features: AUR builds, SBOM generation, fleet management
  let score = 0;
  const usedFeatures = new Set(featureUsage.map(f => f.featureName));

  for (const feature of ADVANCED_FEATURES) {
    if (usedFeatures.has(feature)) {
      score += 8; // 8 points per advanced feature, max 40 points
    }
  }

  return Math.min(40, score);
}

function calculateVersionCurrencyScore(currentVersion: string, latestVersion: string): number {
  // 20 points for running latest version, scaled down for older versions
  if (!currentVersion || !latestVersion) return 10;

  // Simple version comparison (assumes semantic versioning)
  const current = parseVersion(currentVersion);
  const latest = parseVersion(latestVersion);

  if (current.major === latest.major && current.minor === latest.minor) {
    return 20; // Same minor version
  }
  if (current.major === latest.major && latest.minor - current.minor <= 2) {
    return 15; // Within 2 minor versions
  }
  if (current.major === latest.major) {
    return 10; // Same major version
  }
  return 5; // Major version behind
}

function parseVersion(version: string): { major: number; minor: number; patch: number } {
  const match = version.match(/(\d+)\.(\d+)\.(\d+)/);
  if (!match) return { major: 0, minor: 0, patch: 0 };
  return {
    major: parseInt(match[1], 10),
    minor: parseInt(match[2], 10),
    patch: parseInt(match[3], 10),
  };
}

function calculateAdoptionScore(data: HealthDataInput): AdoptionScore {
  const featureBreadthScore = calculateFeatureBreadthScore(data.featureUsage);
  const advancedFeatureScore = calculateAdvancedFeatureScore(data.featureUsage);
  const versionCurrencyScore = calculateVersionCurrencyScore(data.currentOmgVersion, data.latestOmgVersion);

  // Normalize to 0-100
  const rawTotal = featureBreadthScore + advancedFeatureScore + versionCurrencyScore;
  const maxPossible = 40 + 40 + 20; // 100
  const total = Math.round((rawTotal / maxPossible) * 100);

  return {
    total,
    featureBreadthScore,
    advancedFeatureScore,
    versionCurrencyScore,
  };
}

// ============================================================================
// Satisfaction Score Calculation (20% weight)
// ============================================================================

function calculateErrorRateScore(commandEvents: CommandEventRecord[]): number {
  // Lower error rate = higher score
  if (commandEvents.length === 0) return 50; // Default to neutral

  const failedCommands = commandEvents.filter(e => !e.success).length;
  const errorRate = failedCommands / commandEvents.length;

  // 0% errors = 40 points, 10%+ errors = 0 points
  return Math.round(Math.max(0, 40 - (errorRate * 400)));
}

function calculateTimeSavedScore(usageDaily: UsageDailyRecord[], industryAvgMs: number): number {
  // Compare time saved to industry average
  const totalTimeSaved = usageDaily.reduce((sum, d) => sum + d.timeSavedMs, 0);
  const avgDailyTimeSaved = totalTimeSaved / Math.max(1, usageDaily.length);

  // If saving more than industry average = 30 points
  // If saving at industry average = 15 points
  // If saving less = proportionally less
  const ratio = avgDailyTimeSaved / industryAvgMs;

  if (ratio >= 1) return 30;
  return Math.round(ratio * 30);
}

function calculateAchievementProgressScore(total: number, unlocked: number): number {
  // Max 30 points based on achievement completion
  if (total === 0) return 15; // Default if no achievements defined

  const completionRate = unlocked / total;
  return Math.round(completionRate * 30);
}

function calculateSatisfactionScore(data: HealthDataInput): SatisfactionScore {
  const industryAvg = data.industryAvgTimeSavedMs ?? DEFAULT_INDUSTRY_AVG_TIME_SAVED_MS;

  const errorRateScore = calculateErrorRateScore(data.commandEvents);
  const timeSavedScore = calculateTimeSavedScore(data.usageDaily, industryAvg);
  const achievementProgressScore = calculateAchievementProgressScore(
    data.totalAchievements,
    data.unlockedAchievements
  );

  // Normalize to 0-100
  const rawTotal = errorRateScore + timeSavedScore + achievementProgressScore;
  const maxPossible = 40 + 30 + 30; // 100
  const total = Math.round((rawTotal / maxPossible) * 100);

  return {
    total,
    errorRateScore,
    timeSavedScore,
    achievementProgressScore,
  };
}

// ============================================================================
// Support Health Score Calculation (10% weight)
// ============================================================================

function calculateTicketVolumeScore(tickets: SupportTicket[]): number {
  // Recent support tickets (last 30 days) - negative if many
  const thirtyDaysAgo = Date.now() - (30 * 24 * 60 * 60 * 1000);
  const recentTickets = tickets.filter(t => t.createdAt > thirtyDaysAgo);

  // 0 tickets = 40 points, 1 ticket = 30, 2 = 20, 3+ = 10
  const count = recentTickets.length;
  if (count === 0) return 40;
  if (count === 1) return 30;
  if (count === 2) return 20;
  return 10;
}

function calculateResolutionSatisfactionScore(tickets: SupportTicket[]): number {
  // Average satisfaction rating of resolved tickets
  const resolvedWithRating = tickets.filter(
    t => t.status === 'resolved' || t.status === 'closed'
  ).filter(t => t.satisfactionRating !== undefined);

  if (resolvedWithRating.length === 0) return 30; // Default if no rated tickets

  const avgRating = resolvedWithRating.reduce(
    (sum, t) => sum + (t.satisfactionRating ?? 0),
    0
  ) / resolvedWithRating.length;

  // Rating is 1-5, scale to 0-30 points
  return Math.round(((avgRating - 1) / 4) * 30);
}

function calculateNpsContribution(npsScore: number | undefined): number {
  // NPS is -100 to 100, scale to 0-30 points
  if (npsScore === undefined) return 15; // Default to neutral

  // -100 = 0 points, 0 = 15 points, 100 = 30 points
  return Math.round(((npsScore + 100) / 200) * 30);
}

function calculateSupportHealthScore(data: HealthDataInput): SupportHealthScore {
  const tickets = data.supportTickets ?? [];

  const ticketVolumeScore = calculateTicketVolumeScore(tickets);
  const resolutionSatisfactionScore = calculateResolutionSatisfactionScore(tickets);
  const npsContribution = calculateNpsContribution(data.npsScore);

  // Normalize to 0-100
  const rawTotal = ticketVolumeScore + resolutionSatisfactionScore + npsContribution;
  const maxPossible = 40 + 30 + 30; // 100
  const total = Math.round((rawTotal / maxPossible) * 100);

  return {
    total,
    ticketVolumeScore,
    resolutionSatisfactionScore,
    npsScore: npsContribution,
  };
}

// ============================================================================
// Main Health Score Calculation
// ============================================================================

/**
 * Calculate comprehensive customer health score
 * @param data - All available customer health data
 * @returns Health score result with breakdown and churn risk assessment
 */
export function calculateHealthScore(data: HealthDataInput): HealthScoreResult {
  const engagement = calculateEngagementScore(data);
  const adoption = calculateAdoptionScore(data);
  const satisfaction = calculateSatisfactionScore(data);
  const supportHealth = calculateSupportHealthScore(data);

  // Calculate weighted overall score
  const overallScore = Math.round(
    engagement.total * WEIGHTS.engagement +
    adoption.total * WEIGHTS.adoption +
    satisfaction.total * WEIGHTS.satisfaction +
    supportHealth.total * WEIGHTS.supportHealth
  );

  // Determine churn risk based on overall score and specific signals
  const churnRisk = determineChurnRisk(overallScore, data);

  return {
    overallScore,
    engagement,
    adoption,
    satisfaction,
    supportHealth,
    churnRisk,
    calculatedAt: Date.now(),
  };
}

function determineChurnRisk(score: number, data: HealthDataInput): ChurnRisk {
  // Primary assessment based on score
  if (score >= 75) return 'low';
  if (score >= 50) return 'medium';
  if (score >= 25) return 'high';
  return 'critical';
}

// ============================================================================
// Churn Risk Assessment
// ============================================================================

/**
 * Calculate churn risk level based on health score and trends
 * @param score - Current health score result
 * @param trends - Historical health trends
 * @returns 'high', 'medium', or 'low' risk level
 */
export function calculateChurnRisk(
  score: HealthScoreResult,
  trends: HealthTrends
): ChurnRisk {
  // Start with score-based risk
  let riskPoints = 0;

  // Score-based risk (0-40 points)
  if (score.overallScore < 25) riskPoints += 40;
  else if (score.overallScore < 50) riskPoints += 25;
  else if (score.overallScore < 75) riskPoints += 10;

  // Trend-based risk (0-30 points)
  if (trends.engagementTrend === 'declining') riskPoints += 20;
  else if (trends.engagementTrend === 'stable') riskPoints += 5;

  // Inactivity risk (0-20 points)
  if (trends.daysSinceLastActivity > 14) riskPoints += 20;
  else if (trends.daysSinceLastActivity > 7) riskPoints += 10;
  else if (trends.daysSinceLastActivity > 3) riskPoints += 5;

  // Velocity decline (0-10 points)
  if (trends.commandVelocity7d < trends.commandVelocity30d * 0.5) {
    riskPoints += 10; // Recent velocity dropped by >50%
  }

  // Convert points to risk level
  if (riskPoints >= 60) return 'critical';
  if (riskPoints >= 40) return 'high';
  if (riskPoints >= 20) return 'medium';
  return 'low';
}

// ============================================================================
// Intervention Suggestions
// ============================================================================

/**
 * Generate actionable intervention suggestions for customer success team
 * @param healthData - Raw health data input
 * @returns Array of prioritized intervention suggestions
 */
export function getInterventionSuggestions(
  healthData: HealthDataInput
): InterventionSuggestion[] {
  const suggestions: InterventionSuggestion[] = [];
  const score = calculateHealthScore(healthData);

  // Check engagement signals
  const activeDays = healthData.usageDaily.filter(d => d.commandsRun > 0).length;
  if (activeDays < 7) {
    suggestions.push({
      priority: activeDays < 3 ? 'urgent' : 'high',
      category: 'engagement',
      title: 'Low Activity Detected',
      description: `Customer has only been active ${activeDays} days in the last 30 days.`,
      suggestedAction: 'Schedule check-in call to understand usage barriers and provide onboarding support.',
      expectedImpact: 'Re-engagement can increase retention by 40%',
    });
  }

  // Check feature adoption
  const usedFeatures = new Set(healthData.featureUsage.map(f => f.featureName));
  const advancedFeaturesUsed = ADVANCED_FEATURES.filter(f => usedFeatures.has(f));

  if (advancedFeaturesUsed.length === 0) {
    suggestions.push({
      priority: 'medium',
      category: 'adoption',
      title: 'Low Feature Adoption',
      description: 'Customer has not used any advanced features (AUR, SBOM, Fleet, SLSA, Audit).',
      suggestedAction: 'Send targeted email showcasing advanced features relevant to their use case.',
      expectedImpact: 'Feature adoption increases stickiness and reduces churn by 25%',
    });
  }

  // Check version currency
  if (healthData.currentOmgVersion !== healthData.latestOmgVersion) {
    const current = parseVersion(healthData.currentOmgVersion);
    const latest = parseVersion(healthData.latestOmgVersion);

    if (latest.major > current.major || (latest.major === current.major && latest.minor - current.minor > 2)) {
      suggestions.push({
        priority: 'low',
        category: 'adoption',
        title: 'Outdated Version',
        description: `Running ${healthData.currentOmgVersion}, latest is ${healthData.latestOmgVersion}.`,
        suggestedAction: 'Send update notification highlighting new features and security fixes.',
        expectedImpact: 'Updated users report 15% higher satisfaction',
      });
    }
  }

  // Check error rate
  const failedCommands = healthData.commandEvents.filter(e => !e.success).length;
  const errorRate = healthData.commandEvents.length > 0
    ? failedCommands / healthData.commandEvents.length
    : 0;

  if (errorRate > 0.1) {
    suggestions.push({
      priority: 'high',
      category: 'satisfaction',
      title: 'High Error Rate',
      description: `${Math.round(errorRate * 100)}% of commands are failing.`,
      suggestedAction: 'Review error logs and proactively reach out with troubleshooting assistance.',
      expectedImpact: 'Resolving errors can prevent immediate churn',
    });
  }

  // Check support ticket volume
  const recentTickets = (healthData.supportTickets ?? []).filter(
    t => t.createdAt > Date.now() - (30 * 24 * 60 * 60 * 1000)
  );

  if (recentTickets.length >= 3) {
    suggestions.push({
      priority: 'urgent',
      category: 'support',
      title: 'High Support Volume',
      description: `${recentTickets.length} support tickets in the last 30 days.`,
      suggestedAction: 'Escalate to senior support and consider dedicated CSM assignment.',
      expectedImpact: 'High-touch support increases enterprise retention by 35%',
    });
  }

  // Check NPS
  if (healthData.npsScore !== undefined && healthData.npsScore < 0) {
    suggestions.push({
      priority: healthData.npsScore < -30 ? 'urgent' : 'high',
      category: 'satisfaction',
      title: 'Detractor NPS Score',
      description: `NPS score of ${healthData.npsScore} indicates dissatisfaction.`,
      suggestedAction: 'Schedule executive sponsor call to address concerns directly.',
      expectedImpact: 'Converting detractors to passives reduces churn risk by 50%',
    });
  }

  // Check for tier upgrade opportunity (positive intervention)
  if (healthData.tier === 'free' && score.overallScore >= 70) {
    suggestions.push({
      priority: 'medium',
      category: 'retention',
      title: 'Upgrade Opportunity',
      description: 'High-engagement free user showing power user behavior.',
      suggestedAction: 'Initiate conversation about team/enterprise tier benefits.',
      expectedImpact: 'Conversion increases LTV and customer commitment',
    });
  }

  // Check achievement progress stagnation
  if (healthData.totalAchievements > 0) {
    const completionRate = healthData.unlockedAchievements / healthData.totalAchievements;
    if (completionRate < 0.2 && activeDays > 7) {
      suggestions.push({
        priority: 'low',
        category: 'engagement',
        title: 'Achievement Progress Stalled',
        description: 'Active user with low achievement completion rate.',
        suggestedAction: 'Send gamification reminder showing next achievable milestones.',
        expectedImpact: 'Gamification increases engagement by 20%',
      });
    }
  }

  // Sort by priority
  const priorityOrder: Record<string, number> = { urgent: 0, high: 1, medium: 2, low: 3 };
  suggestions.sort((a, b) => priorityOrder[a.priority] - priorityOrder[b.priority]);

  return suggestions;
}

// ============================================================================
// Churn Probability Prediction
// ============================================================================

export interface ChurnHistoryRecord {
  date: string;
  healthScore: number;
  daysActive: number;
  commandsRun: number;
  errorRate: number;
  featuresUsed: number;
  daysSinceLastActivity: number;
  supportTickets: number;
  tier: 'free' | 'team' | 'enterprise';
  churned: boolean; // Target variable for training
}

/**
 * Predict churn probability using logistic regression baseline
 *
 * Uses a simplified logistic regression model with the following features:
 * - Health score (negative correlation with churn)
 * - Days inactive (positive correlation with churn)
 * - Error rate (positive correlation with churn)
 * - Feature adoption (negative correlation with churn)
 * - Support tickets (positive correlation with churn)
 * - Tier (enterprise least likely to churn)
 *
 * @param history - Historical data for the customer
 * @returns Churn prediction with probability, confidence, and risk factors
 */
export function predictChurnProbability(
  history: ChurnHistoryRecord[]
): ChurnPrediction {
  if (history.length === 0) {
    return {
      probability: 0.5,
      confidence: 0.1,
      riskFactors: ['Insufficient data for prediction'],
      protectiveFactors: [],
    };
  }

  // Use most recent record for current state
  const current = history[history.length - 1];

  // Feature weights (pre-trained coefficients for logistic regression)
  // These would normally be trained on historical churn data
  const weights = {
    intercept: -0.5,
    healthScore: -0.03,        // Lower health = higher churn
    daysInactive: 0.08,        // More inactive days = higher churn
    errorRate: 2.0,            // Higher error rate = higher churn
    featuresUsed: -0.15,       // More features = lower churn
    supportTickets: 0.2,       // More tickets = higher churn
    tierFree: 0.5,             // Free tier more likely to churn
    tierTeam: 0.1,             // Team tier slightly likely
    tierEnterprise: -0.3,      // Enterprise least likely
  };

  // Calculate logit (log-odds)
  let logit = weights.intercept;
  logit += weights.healthScore * current.healthScore;
  logit += weights.daysInactive * current.daysSinceLastActivity;
  logit += weights.errorRate * current.errorRate;
  logit += weights.featuresUsed * current.featuresUsed;
  logit += weights.supportTickets * current.supportTickets;

  // Tier one-hot encoding
  if (current.tier === 'free') logit += weights.tierFree;
  else if (current.tier === 'team') logit += weights.tierTeam;
  else logit += weights.tierEnterprise;

  // Apply sigmoid function to get probability
  const probability = 1 / (1 + Math.exp(-logit));

  // Calculate confidence based on data quality
  const dataPoints = history.length;
  const confidence = Math.min(0.9, 0.3 + (dataPoints * 0.02));

  // Identify risk and protective factors
  const riskFactors: string[] = [];
  const protectiveFactors: string[] = [];

  if (current.healthScore < 50) {
    riskFactors.push(`Low health score (${current.healthScore})`);
  } else if (current.healthScore >= 75) {
    protectiveFactors.push(`High health score (${current.healthScore})`);
  }

  if (current.daysSinceLastActivity > 7) {
    riskFactors.push(`${current.daysSinceLastActivity} days since last activity`);
  } else if (current.daysSinceLastActivity <= 1) {
    protectiveFactors.push('Active within the last day');
  }

  if (current.errorRate > 0.1) {
    riskFactors.push(`High error rate (${Math.round(current.errorRate * 100)}%)`);
  } else if (current.errorRate < 0.02) {
    protectiveFactors.push('Very low error rate');
  }

  if (current.featuresUsed <= 2) {
    riskFactors.push('Low feature adoption');
  } else if (current.featuresUsed >= 5) {
    protectiveFactors.push('High feature adoption');
  }

  if (current.supportTickets >= 3) {
    riskFactors.push(`${current.supportTickets} support tickets recently`);
  } else if (current.supportTickets === 0) {
    protectiveFactors.push('No recent support issues');
  }

  if (current.tier === 'enterprise') {
    protectiveFactors.push('Enterprise tier commitment');
  } else if (current.tier === 'free') {
    riskFactors.push('Free tier - no financial commitment');
  }

  // Check for declining trend
  if (history.length >= 7) {
    const recentScores = history.slice(-7).map(h => h.healthScore);
    const olderScores = history.slice(-14, -7).map(h => h.healthScore);

    if (olderScores.length >= 7) {
      const recentAvg = recentScores.reduce((a, b) => a + b, 0) / recentScores.length;
      const olderAvg = olderScores.reduce((a, b) => a + b, 0) / olderScores.length;

      if (recentAvg < olderAvg * 0.9) {
        riskFactors.push('Declining health score trend');
      } else if (recentAvg > olderAvg * 1.1) {
        protectiveFactors.push('Improving health score trend');
      }
    }
  }

  return {
    probability: Math.round(probability * 1000) / 1000,
    confidence: Math.round(confidence * 100) / 100,
    riskFactors,
    protectiveFactors,
  };
}

// ============================================================================
// Utility Functions
// ============================================================================

/**
 * Calculate health trends from historical data
 * @param scoreHistory - Array of historical health scores
 * @param usageDaily - Daily usage records
 * @returns Health trend analysis
 */
export function calculateHealthTrends(
  scoreHistory: Array<{ date: string; score: number }>,
  usageDaily: UsageDailyRecord[]
): HealthTrends {
  const now = Date.now();
  const sevenDaysAgo = now - (7 * 24 * 60 * 60 * 1000);
  const thirtyDaysAgo = now - (30 * 24 * 60 * 60 * 1000);

  // Calculate command velocity
  const last7Days = usageDaily.filter(d => new Date(d.date).getTime() > sevenDaysAgo);
  const last30Days = usageDaily.filter(d => new Date(d.date).getTime() > thirtyDaysAgo);

  const commandVelocity7d = last7Days.reduce((sum, d) => sum + d.commandsRun, 0) / 7;
  const commandVelocity30d = last30Days.reduce((sum, d) => sum + d.commandsRun, 0) / 30;

  // Determine engagement trend
  let engagementTrend: VelocityTrend = 'stable';
  if (commandVelocity7d > commandVelocity30d * 1.1) {
    engagementTrend = 'growing';
  } else if (commandVelocity7d < commandVelocity30d * 0.9) {
    engagementTrend = 'declining';
  }

  // Calculate days active in last 30
  const daysActive30d = last30Days.filter(d => d.commandsRun > 0).length;

  // Calculate days since last activity
  const sortedUsage = [...usageDaily].sort((a, b) => b.date.localeCompare(a.date));
  const lastActiveDate = sortedUsage.find(d => d.commandsRun > 0)?.date;
  const daysSinceLastActivity = lastActiveDate
    ? Math.floor((now - new Date(lastActiveDate).getTime()) / (24 * 60 * 60 * 1000))
    : 30; // Assume max if no activity

  return {
    scoreHistory,
    engagementTrend,
    commandVelocity7d: Math.round(commandVelocity7d * 10) / 10,
    commandVelocity30d: Math.round(commandVelocity30d * 10) / 10,
    daysActive30d,
    daysSinceLastActivity,
  };
}

/**
 * Get a human-readable health status label
 * @param score - Health score (0-100)
 * @returns Status label
 */
export function getHealthStatusLabel(score: number): string {
  if (score >= 80) return 'Excellent';
  if (score >= 60) return 'Good';
  if (score >= 40) return 'Fair';
  if (score >= 20) return 'Poor';
  return 'Critical';
}

/**
 * Get color for health score visualization
 * @param score - Health score (0-100)
 * @returns Hex color code
 */
export function getHealthScoreColor(score: number): string {
  if (score >= 80) return '#10b981'; // Green
  if (score >= 60) return '#22c55e'; // Light green
  if (score >= 40) return '#f59e0b'; // Amber
  if (score >= 20) return '#ef4444'; // Red
  return '#dc2626'; // Dark red
}

/**
 * Get color for churn risk level
 * @param risk - Churn risk level
 * @returns Hex color code
 */
export function getChurnRiskColor(risk: ChurnRisk): string {
  switch (risk) {
    case 'low': return '#10b981';     // Green
    case 'medium': return '#f59e0b';  // Amber
    case 'high': return '#ef4444';    // Red
    case 'critical': return '#dc2626'; // Dark red
  }
}
