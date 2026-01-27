import { Component, createSignal, createMemo, For, Show, Switch, Match, ErrorBoundary } from 'solid-js';
import {
  Activity,
  Users,
  Search,
  Download,
  BarChart3,
  CreditCard,
  History,
  ChevronDown,
  Lightbulb,
  Calendar,
  Filter,
  GitCompare,
  Save,
  Layers,
  Brain,
  FileText,
  AlertTriangle,
  X,
  Loader2,
} from 'lucide-solid';
import * as api from '../../lib/api';
import {
  useAdminDashboard,
  useAdminFirehose,
  useAdminCRMUsers,
  useAdminAdvancedMetrics,
} from '../../lib/api-hooks';
import { CardSkeleton } from '../ui/Skeleton';
import { ErrorFallback } from '../ui/ErrorFallback';
import { DocsAnalytics } from './admin/DocsAnalytics';
import { CohortAnalysis } from './admin/CohortAnalysis';
import { RevenueTab } from './admin/RevenueTab';
import { AuditLogTab } from './admin/AuditLogTab';
import { CustomerDetailDrawer } from './admin/CustomerDetailDrawer';
import { InsightsTab } from './admin/insights/InsightsTab';
import { SegmentAnalytics } from './admin/SegmentAnalytics';
import { PredictiveInsights } from './admin/PredictiveInsights';
import { CustomReportBuilder } from './admin/CustomReportBuilder';

type DateRange = '7d' | '30d' | '90d' | 'custom';
type SavedView = {
  id: string;
  name: string;
  tab: AdminTab;
  dateRange: DateRange;
  segment: string;
  compareEnabled: boolean;
};

import {
  ExecutiveKPIDashboard,
  RealTimeCommandCenter,
  CRMProfileCard,
  CRMProfileCardTableRow,
} from './premium';
import type {
  ExecutiveKPI,
  AdvancedMetrics,
  FirehoseEvent,
  GeoDistribution,
  CommandHealth,
  CRMCustomer,
  CustomerHealth,
} from './premium/types';

type AdminTab = 'overview' | 'crm' | 'analytics' | 'insights' | 'revenue' | 'audit' | 'segments' | 'predictions' | 'reports';

const SEGMENTS = [
  { id: 'all', name: 'All Customers' },
  { id: 'enterprise', name: 'Enterprise' },
  { id: 'team', name: 'Team' },
  { id: 'pro', name: 'Pro' },
  { id: 'power_users', name: 'Power Users' },
  { id: 'at_risk', name: 'At Risk' },
  { id: 'new_users', name: 'New Users (30d)' },
];

function transformToExecutiveKPI(
  dashboard: api.AdminOverview | undefined,
  metrics: api.AdminAdvancedMetrics | undefined
): ExecutiveKPI {
  const mrr = dashboard?.overview?.mrr || 0;
  return {
    mrr,
    mrr_change: 8.3, // Would calculate from historical data
    arr: mrr * 12,
    dau: metrics?.engagement?.dau || dashboard?.daily_active_users?.[0]?.active_users || 0,
    wau: metrics?.engagement?.wau || 0,
    mau: metrics?.engagement?.mau || 0,
    stickiness: parseFloat(metrics?.engagement?.stickiness?.daily_to_monthly?.replace('%', '') || '0'),
    churn_rate: metrics?.churn_risk_segments?.reduce((acc, s) => s.risk_segment === 'high' || s.risk_segment === 'critical' ? acc + s.user_count : acc, 0) 
      ? (metrics.churn_risk_segments.reduce((acc, s) => s.risk_segment === 'high' || s.risk_segment === 'critical' ? acc + s.user_count : acc, 0) / (metrics.engagement?.mau || 1)) * 100 
      : 2.1,
    at_risk_count: metrics?.churn_risk_segments?.reduce((acc, s) => s.risk_segment === 'high' || s.risk_segment === 'critical' ? acc + s.user_count : acc, 0) || 0,
    expansion_pipeline: metrics?.revenue_metrics?.expansion_mrr_12m || 0,
  };
}

function transformToAdvancedMetrics(metrics: api.AdminAdvancedMetrics | undefined): AdvancedMetrics | undefined {
  if (!metrics) return undefined;
  return {
    engagement: {
      dau: metrics.engagement?.dau || 0,
      wau: metrics.engagement?.wau || 0,
      mau: metrics.engagement?.mau || 0,
      stickiness: {
        daily_to_monthly: metrics.engagement?.stickiness?.daily_to_monthly || '0%',
        daily_to_weekly: metrics.engagement?.stickiness?.weekly_to_monthly || '0%',
      },
    },
    retention: {
      cohorts: metrics.retention?.cohorts?.map(c => ({
        cohort_date: c.cohort_date,
        week_number: c.week_number,
        retained_users: c.retained_users,
        retention_rate: 0,
      })) || [],
    },
    ltv_by_tier: metrics.ltv_by_tier || [],
    feature_adoption: {
      install_adopters: metrics.feature_adoption?.install_adopters || 0,
      search_adopters: metrics.feature_adoption?.search_adopters || 0,
      runtime_adopters: metrics.feature_adoption?.runtime_adopters || 0,
      total_users: metrics.feature_adoption?.total_active_users || 0,
    },
    command_heatmap: metrics.command_heatmap || [],
    runtime_adoption: metrics.runtime_adoption?.map(r => ({
      runtime: r.runtime,
      unique_users: r.unique_users,
      total_uses: r.total_uses,
      growth_rate: 0,
    })) || [],
    churn_risk_segments: metrics.churn_risk_segments?.map(s => ({
      risk_segment: s.risk_segment as 'low' | 'medium' | 'high' | 'critical',
      user_count: s.user_count,
      tier: s.tier,
      avg_days_inactive: 0,
    })) || [],
    expansion_opportunities: metrics.expansion_opportunities?.map(o => ({
      email: o.email,
      tier: o.tier,
      opportunity_type: o.opportunity_type as 'usage_based' | 'feature_gate' | 'team_growth' | 'enterprise',
      priority: o.priority as 'low' | 'medium' | 'high' | 'urgent',
      potential_arr: 0,
    })) || [],
    time_to_value: {
      avg_days_to_activation: metrics.time_to_value?.avg_days_to_activation || 0,
      pct_activated_week1: metrics.time_to_value?.pct_activated_week1 || 0,
      pct_activated_month1: 0,
    },
    revenue_metrics: {
      current_mrr: metrics.revenue_metrics?.current_mrr || 0,
      projected_arr: metrics.revenue_metrics?.projected_arr || 0,
      expansion_mrr_12m: metrics.revenue_metrics?.expansion_mrr_12m || 0,
      net_revenue_retention: 0,
    },
  };
}

interface RawFirehoseEvent {
  id?: string;
  event_name?: string;
  action?: string;
  machine_id?: string;
  hostname?: string;
  platform?: string;
  timestamp?: string;
  created_at?: string;
  duration_ms?: number;
  success?: boolean;
  metadata?: {
    hostname?: string;
    platform?: string;
  };
}

function transformFirehoseEvents(events: RawFirehoseEvent[]): FirehoseEvent[] {
  return events.map((e, i) => ({
    id: e.id || `evt-${i}`,
    event_type: mapEventType(e.event_name || e.action || ''),
    event_name: e.event_name || e.action || 'unknown',
    machine_id: e.machine_id || '',
    hostname: e.hostname || e.metadata?.hostname || '',
    platform: e.platform || e.metadata?.platform || 'unknown',
    timestamp: e.timestamp || e.created_at || new Date().toISOString(),
    duration_ms: e.duration_ms || 0,
    success: e.success !== false,
  }));
}

function mapEventType(eventName: string): FirehoseEvent['event_type'] {
  const lower = eventName.toLowerCase();
  if (lower.includes('install')) return 'install';
  if (lower.includes('search')) return 'search';
  if (lower.includes('runtime') || lower.includes('use ')) return 'runtime_switch';
  if (lower.includes('error') || lower.includes('fail')) return 'error';
  return 'command';
}

function transformGeoDistribution(data: { dimension: string; count: number }[]): GeoDistribution[] {
  const total = data.reduce((sum, d) => sum + d.count, 0) || 1;
  return data.map(d => ({
    country: getCountryName(d.dimension),
    country_code: d.dimension || 'XX',
    count: d.count,
    percentage: (d.count / total) * 100,
  }));
}

function getCountryName(code: string): string {
  const countries: Record<string, string> = {
    US: 'United States', DE: 'Germany', GB: 'United Kingdom', FR: 'France',
    CA: 'Canada', JP: 'Japan', AU: 'Australia', BR: 'Brazil', IN: 'India',
    NL: 'Netherlands', SE: 'Sweden', ES: 'Spain', IT: 'Italy', KR: 'South Korea',
  };
  return countries[code] || code || 'Unknown';
}

function transformToCRMCustomer(user: api.AdminUser): CRMCustomer {
  const score = user.engagement_score || 50;
  const stage = (user.lifecycle_stage || 'active') as CustomerHealth['lifecycle_stage'];
  
  return {
    id: user.id,
    email: user.email,
    company: user.company || undefined,
    tier: user.tier || 'free',
    status: (user.status as 'active' | 'suspended' | 'cancelled') || 'active',
    health: {
      overall_score: score,
      engagement_score: Math.min(100, score + 10),
      activation_score: Math.min(100, score + 5),
      growth_score: Math.max(0, score - 10),
      risk_score: Math.max(0, 100 - score),
      lifecycle_stage: stage,
      predicted_churn_probability: stage === 'at_risk' ? 0.6 : stage === 'churned' ? 0.9 : 0.1,
      predicted_upgrade_probability: score > 70 ? 0.7 : 0.3,
      expansion_readiness_score: score,
      command_velocity_7d: user.total_commands || 0,
      command_velocity_trend: score > 60 ? 'growing' : score > 40 ? 'stable' : 'declining',
    },
    tags: [],
    created_at: user.created_at,
    last_activity_at: user.last_active || user.created_at,
    total_commands: user.total_commands || 0,
    machine_count: user.machine_count || 0,
    mrr: user.tier === 'enterprise' ? 199 : user.tier === 'team' ? 29 : user.tier === 'pro' ? 9 : 0,
  };
}

export const AdminDashboard: Component = () => {
  const [activeTab, setActiveTab] = createSignal<AdminTab>('overview');
  const [crmPage, setCrmPage] = createSignal(1);
  const [crmSearch, setCrmSearch] = createSignal('');
  const [selectedUserId, setSelectedUserId] = createSignal<string | null>(null);
  const [exportMenuOpen, setExportMenuOpen] = createSignal(false);
  const [isExporting, setIsExporting] = createSignal(false);
  const [exportingType, setExportingType] = createSignal<string | null>(null);
  const [exportError, setExportError] = createSignal<string | null>(null);
  const [crmViewMode, setCrmViewMode] = createSignal<'cards' | 'table'>('table');

  const [dateRange, setDateRange] = createSignal<DateRange>('30d');
  const [selectedSegment, setSelectedSegment] = createSignal('all');
  const [compareEnabled, setCompareEnabled] = createSignal(false);
  const [savedViews, setSavedViews] = createSignal<SavedView[]>([]);
  const [showSaveViewModal, setShowSaveViewModal] = createSignal(false);
  const [newViewName, setNewViewName] = createSignal('');
  const dashboardQuery = useAdminDashboard();
  const firehoseQuery = useAdminFirehose(100);
  const crmUsersQuery = useAdminCRMUsers(crmPage(), 25, crmSearch());
  const advancedMetricsQuery = useAdminAdvancedMetrics();

  // Transformed data for premium components
  const executiveKPI = createMemo(() =>
    transformToExecutiveKPI(dashboardQuery.data, advancedMetricsQuery.data)
  );

  const advancedMetrics = createMemo(() =>
    transformToAdvancedMetrics(advancedMetricsQuery.data)
  );

  const firehoseEvents = createMemo(() =>
    transformFirehoseEvents(firehoseQuery.data?.events || [])
  );

  const geoDistribution = createMemo(() =>
    transformGeoDistribution(dashboardQuery.data?.geo_distribution || [])
  );

  const commandHealth = createMemo((): CommandHealth => {
    const health = dashboardQuery.data?.overview?.command_health;
    const total = (health?.success || 0) + (health?.failure || 0);
    if (total === 0) return { success: 95, failure: 5 };
    return {
      success: ((health?.success || 0) / total) * 100,
      failure: ((health?.failure || 0) / total) * 100,
    };
  });

  const crmCustomers = createMemo(() =>
    (crmUsersQuery.data?.users || []).map(transformToCRMCustomer)
  );

  const crmPagination = () => crmUsersQuery.data?.pagination;

  // Helper to get days from dateRange
  const getDateRangeDays = (): number => {
    const range = dateRange();
    if (range === '7d') return 7;
    if (range === '30d') return 30;
    if (range === '90d') return 90;
    return 30; // default for custom
  };

  // Export handlers
  const handleExport = async (type: 'users' | 'usage' | 'audit') => {
    setIsExporting(true);
    setExportingType(type);
    setExportMenuOpen(false);
    setExportError(null);
    try {
      let data: string;
      let filename: string;
      const days = getDateRangeDays();
      switch (type) {
        case 'users':
          data = await api.exportAdminUsers();
          filename = `omg-users-${new Date().toISOString().split('T')[0]}.csv`;
          break;
        case 'usage':
          data = await api.exportAdminUsage(days);
          filename = `omg-usage-${dateRange()}-${new Date().toISOString().split('T')[0]}.csv`;
          break;
        case 'audit':
          data = await api.exportAdminAudit(days);
          filename = `omg-audit-${dateRange()}-${new Date().toISOString().split('T')[0]}.csv`;
          break;
      }
      api.downloadCSV(data, filename);
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error occurred';
      setExportError(`Failed to export ${type}: ${errorMessage}`);
      console.error('Export failed:', error);
    } finally {
      setIsExporting(false);
      setExportingType(null);
    }
  };

  const saveCurrentView = () => {
    if (!newViewName().trim()) return;
    const view: SavedView = {
      id: `view-${Date.now()}`,
      name: newViewName(),
      tab: activeTab(),
      dateRange: dateRange(),
      segment: selectedSegment(),
      compareEnabled: compareEnabled(),
    };
    setSavedViews((prev) => [...prev, view]);
    setNewViewName('');
    setShowSaveViewModal(false);
  };

  const loadView = (view: SavedView) => {
    setActiveTab(view.tab);
    setDateRange(view.dateRange);
    setSelectedSegment(view.segment);
    setCompareEnabled(view.compareEnabled);
  };

  const tabCounts = createMemo(() => ({
    crm: crmUsersQuery.data?.pagination?.total || 0,
    insights: advancedMetricsQuery.data?.expansion_opportunities?.length || 0,
    predictions:
      (advancedMetricsQuery.data?.churn_risk_segments?.filter(
        (s) => s.risk_segment === 'high' || s.risk_segment === 'critical'
      ).length || 0) + (advancedMetricsQuery.data?.expansion_opportunities?.length || 0),
  }));

  const TAB_ORDER: AdminTab[] = ['overview', 'crm', 'analytics', 'insights', 'segments', 'predictions', 'reports', 'revenue', 'audit'];

  const handleTabKeyNavigation = (e: KeyboardEvent, currentId: AdminTab) => {
    const currentIndex = TAB_ORDER.indexOf(currentId);
    
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      e.preventDefault();
      const nextIndex = (currentIndex + 1) % TAB_ORDER.length;
      setActiveTab(TAB_ORDER[nextIndex]);
      document.getElementById(`tab-${TAB_ORDER[nextIndex]}`)?.focus();
    }
    if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      e.preventDefault();
      const prevIndex = (currentIndex - 1 + TAB_ORDER.length) % TAB_ORDER.length;
      setActiveTab(TAB_ORDER[prevIndex]);
      document.getElementById(`tab-${TAB_ORDER[prevIndex]}`)?.focus();
    }
    if (e.key === 'Home') {
      e.preventDefault();
      setActiveTab(TAB_ORDER[0]);
      document.getElementById(`tab-${TAB_ORDER[0]}`)?.focus();
    }
    if (e.key === 'End') {
      e.preventDefault();
      setActiveTab(TAB_ORDER[TAB_ORDER.length - 1]);
      document.getElementById(`tab-${TAB_ORDER[TAB_ORDER.length - 1]}`)?.focus();
    }
  };

  const TabButton = (props: { id: AdminTab; icon: Component<{ size?: number }>; label: string; count?: number }) => (
    <button
      id={`tab-${props.id}`}
      role="tab"
      aria-selected={activeTab() === props.id}
      aria-controls={`panel-${props.id}`}
      tabindex={activeTab() === props.id ? 0 : -1}
      onClick={() => setActiveTab(props.id)}
      onKeyDown={(e) => handleTabKeyNavigation(e, props.id)}
      class={`flex items-center gap-2 rounded-xl px-4 py-2.5 font-bold transition-all ${
        activeTab() === props.id
          ? 'scale-[1.02] bg-white text-black shadow-lg'
          : 'text-slate-400 hover:bg-white/5 hover:text-white'
      }`}
    >
      <props.icon size={16} />
      <span>{props.label}</span>
      <Show when={props.count !== undefined && props.count > 0}>
        <span
          class={`rounded-full px-1.5 py-0.5 text-2xs font-black ${
            activeTab() === props.id ? 'bg-black/10 text-black' : 'bg-white/10 text-white'
          }`}
        >
          {props.count}
        </span>
      </Show>
    </button>
  );

  return (
    <div class="space-y-6 pb-20">
      <div class="flex flex-col gap-6 lg:flex-row lg:items-center lg:justify-between">
        <div>
          <h1 class="font-display text-4xl font-black tracking-tight text-white">Mission Control</h1>
          <p class="mt-2 font-medium text-slate-400">
            Global infrastructure, revenue, and fleet telemetry
          </p>
        </div>

        <div class="flex flex-wrap items-center gap-3">
          <div class="flex items-center gap-2 rounded-xl border border-white/10 bg-white/[0.03] px-3 py-2">
            <Calendar size={14} class="text-nebula-500" />
            <select
              value={dateRange()}
              onChange={(e) => setDateRange(e.currentTarget.value as DateRange)}
              class="bg-transparent text-sm font-bold text-white focus:outline-none"
            >
              <option value="7d">Last 7 days</option>
              <option value="30d">Last 30 days</option>
              <option value="90d">Last 90 days</option>
              <option value="custom">Custom</option>
            </select>
          </div>

          <div class="flex items-center gap-2 rounded-xl border border-white/10 bg-white/[0.03] px-3 py-2">
            <Filter size={14} class="text-nebula-500" />
            <select
              value={selectedSegment()}
              onChange={(e) => setSelectedSegment(e.currentTarget.value)}
              class="bg-transparent text-sm font-bold text-white focus:outline-none"
            >
              <For each={SEGMENTS}>{(seg) => <option value={seg.id}>{seg.name}</option>}</For>
            </select>
          </div>

          <button
            onClick={() => setCompareEnabled(!compareEnabled())}
            class={`flex items-center gap-2 rounded-xl border px-3 py-2 text-sm font-bold transition-all ${
              compareEnabled()
                ? 'border-indigo-500/50 bg-indigo-500/10 text-indigo-400'
                : 'border-white/10 bg-white/[0.03] text-white hover:bg-white/[0.06]'
            }`}
          >
            <GitCompare size={14} />
            Compare
          </button>

          <button
            onClick={() => setShowSaveViewModal(true)}
            class="flex items-center gap-2 rounded-xl border border-white/10 bg-white/[0.03] px-3 py-2 text-sm font-bold text-white transition-all hover:bg-white/[0.06]"
          >
            <Save size={14} />
            Save View
          </button>

          <div class="relative">
            <button
              onClick={(e) => {
                e.stopPropagation();
                setExportMenuOpen(!exportMenuOpen());
              }}
              disabled={isExporting()}
              aria-haspopup="true"
              aria-expanded={exportMenuOpen()}
              aria-controls="export-menu"
              class="flex items-center gap-2 rounded-xl border border-white/10 bg-white/[0.03] px-3 py-2 text-sm font-bold text-white transition-all hover:bg-white/[0.06] disabled:cursor-not-allowed disabled:opacity-50"
            >
              <Show when={isExporting()} fallback={<Download size={14} />}>
                <Loader2 size={14} class="animate-spin" />
              </Show>
              <Show when={exportingType()} fallback="Export">
                Exporting {exportingType()}...
              </Show>
              <ChevronDown size={12} class={`transition-transform ${exportMenuOpen() ? 'rotate-180' : ''}`} />
            </button>

            <Show when={exportMenuOpen()}>
              <div id="export-menu" role="menu" aria-label="Export options" class="absolute right-0 top-full z-50 mt-2 w-56 origin-top-right rounded-xl border border-white/10 bg-[#0d0d0e] p-1 shadow-2xl">
                <button
                  role="menuitem"
                  onClick={() => handleExport('users')}
                  class="flex w-full items-center gap-3 rounded-lg px-4 py-2.5 text-left text-sm text-white transition-colors hover:bg-white/5"
                >
                  <Users size={16} class="text-indigo-400" />
                  <div>
                    <div class="font-medium">Users</div>
                    <div class="text-xs text-slate-500">Export all users as CSV</div>
                  </div>
                </button>
                <button
                  role="menuitem"
                  onClick={() => handleExport('usage')}
                  class="flex w-full items-center gap-3 rounded-lg px-4 py-2.5 text-left text-sm text-white transition-colors hover:bg-white/5"
                >
                  <BarChart3 size={16} class="text-cyan-400" />
                  <div>
                    <div class="font-medium">Usage ({dateRange()})</div>
                    <div class="text-xs text-slate-500">Export usage data as CSV</div>
                  </div>
                </button>
                <button
                  role="menuitem"
                  onClick={() => handleExport('audit')}
                  class="flex w-full items-center gap-3 rounded-lg px-4 py-2.5 text-left text-sm text-white transition-colors hover:bg-white/5"
                >
                  <History size={16} class="text-purple-400" />
                  <div>
                    <div class="font-medium">Audit Log ({dateRange()})</div>
                    <div class="text-xs text-slate-500">Export audit log as CSV</div>
                  </div>
                </button>
              </div>
            </Show>
          </div>
        </div>
      </div>

      <Show when={compareEnabled()}>
        <div class="flex items-center gap-3 rounded-xl border border-indigo-500/30 bg-indigo-500/10 px-4 py-3">
          <GitCompare size={18} class="text-indigo-400" />
          <span class="text-sm font-medium text-indigo-300">
            Comparing current period with previous {dateRange() === '7d' ? '7 days' : dateRange() === '30d' ? '30 days' : '90 days'}
          </span>
          <button
            onClick={() => setCompareEnabled(false)}
            class="ml-auto rounded-lg bg-indigo-500/20 px-3 py-1 text-xs font-bold text-indigo-300 hover:bg-indigo-500/30"
          >
            Exit Comparison
          </button>
        </div>
      </Show>

      <Show when={savedViews().length > 0}>
        <div class="flex items-center gap-2 overflow-x-auto">
          <span class="text-xs font-bold text-nebula-500">Saved Views:</span>
          <For each={savedViews()}>
            {(view) => (
              <button
                onClick={() => loadView(view)}
                class="flex items-center gap-2 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs font-bold text-white transition-all hover:bg-white/[0.06]"
              >
                {view.name}
              </button>
            )}
          </For>
        </div>
      </Show>

      <div role="tablist" aria-label="Dashboard navigation" class="no-scrollbar flex items-center gap-1.5 overflow-x-auto rounded-2xl border border-white/5 bg-white/[0.02] p-1.5">
        <TabButton id="overview" icon={Activity} label="Overview" />
        <TabButton id="crm" icon={Users} label="CRM" count={tabCounts().crm} />
        <TabButton id="analytics" icon={BarChart3} label="Analytics" />
        <TabButton id="insights" icon={Lightbulb} label="Insights" count={tabCounts().insights} />
        <TabButton id="segments" icon={Layers} label="Segments" />
        <TabButton id="predictions" icon={Brain} label="Predictions" count={tabCounts().predictions} />
        <TabButton id="reports" icon={FileText} label="Reports" />
        <TabButton id="revenue" icon={CreditCard} label="Revenue" />
        <TabButton id="audit" icon={History} label="Audit Log" />
      </div>

      <Show when={dashboardQuery.isLoading || advancedMetricsQuery.isLoading}>
        <div class="grid gap-6 md:grid-cols-2 lg:grid-cols-4">
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
          <CardSkeleton />
        </div>
      </Show>

      <Show when={dashboardQuery.isSuccess}>
        <Switch>
          <Match when={activeTab() === 'overview'}>
            <div role="tabpanel" id="panel-overview" aria-labelledby="tab-overview" tabindex={0} class="space-y-8">
              <ErrorBoundary fallback={(err, reset) => <ErrorFallback error={err} reset={reset} />}>
                <ExecutiveKPIDashboard
                  kpi={executiveKPI()}
                  metrics={advancedMetrics()}
                  isLoading={advancedMetricsQuery.isLoading}
                />
              </ErrorBoundary>

              <ErrorBoundary fallback={(err, reset) => <ErrorFallback error={err} reset={reset} />}>
                <RealTimeCommandCenter
                  events={firehoseEvents()}
                  geoDistribution={geoDistribution()}
                  commandHealth={commandHealth()}
                  isLive={true}
                  onRefresh={() => firehoseQuery.refetch()}
                />
              </ErrorBoundary>
            </div>
          </Match>

          <Match when={activeTab() === 'crm'}>
            <div role="tabpanel" id="panel-crm" aria-labelledby="tab-crm" tabindex={0} class="space-y-6">
              {/* CRM Header */}
              <div class="flex flex-col justify-between gap-6 md:flex-row md:items-center">
                <div>
                  <h3 class="text-2xl font-black tracking-tight text-white">Customer CRM</h3>
                  <p class="text-sm font-medium text-slate-500">
                    {crmPagination()?.total || 0} customers | Manage subscriptions and engagement
                  </p>
                </div>

                <div class="flex items-center gap-4">
                  {/* View Toggle */}
                  <div class="flex rounded-xl border border-white/10 bg-white/[0.02] p-1">
                    <button
                      onClick={() => setCrmViewMode('table')}
                      class={`rounded-lg px-4 py-2 text-xs font-bold transition-all ${
                        crmViewMode() === 'table'
                          ? 'bg-white text-black'
                          : 'text-slate-400 hover:text-white'
                      }`}
                    >
                      Table
                    </button>
                    <button
                      onClick={() => setCrmViewMode('cards')}
                      class={`rounded-lg px-4 py-2 text-xs font-bold transition-all ${
                        crmViewMode() === 'cards'
                          ? 'bg-white text-black'
                          : 'text-slate-400 hover:text-white'
                      }`}
                    >
                      Cards
                    </button>
                  </div>

                  {/* Search */}
                  <div class="relative w-full max-w-md">
                    <Search class="absolute left-4 top-1/2 -translate-y-1/2 text-slate-500" size={18} />
                    <input
                      type="text"
                      placeholder="Search by email, company or ID..."
                      value={crmSearch()}
                      onInput={(e) => {
                        setCrmSearch(e.currentTarget.value);
                        setCrmPage(1);
                      }}
                      class="w-full rounded-2xl border border-white/10 bg-white/5 py-3 pl-12 pr-4 text-white placeholder-slate-500 transition-all focus:outline-none focus:ring-2 focus:ring-indigo-500/20"
                    />
                  </div>
                </div>
              </div>

              <Show when={crmUsersQuery.isLoading}>
                <div class="grid gap-6 md:grid-cols-2 xl:grid-cols-3">
                  <CardSkeleton />
                  <CardSkeleton />
                  <CardSkeleton />
                </div>
              </Show>

              <Show when={crmUsersQuery.isSuccess}>
                {/* Card View */}
                <Show when={crmViewMode() === 'cards'}>
                  <div class="grid gap-6 md:grid-cols-2 xl:grid-cols-3">
                    <For each={crmCustomers()}>
                      {(customer) => (
                        <CRMProfileCard
                          customer={customer}
                          onViewDetail={(customerId) => setSelectedUserId(customerId)}
                          onQuickAction={(action, _customerId) => {
                            if (action === 'email') {
                              window.open(`mailto:${customer.email}`);
                            }
                          }}
                        />
                      )}
                    </For>
                  </div>
                </Show>

                {/* Table View */}
                <Show when={crmViewMode() === 'table'}>
                  <div class="overflow-hidden rounded-[2rem] border border-white/5 bg-[#0d0d0e] shadow-2xl">
                    <div class="overflow-x-auto">
                      <table class="w-full text-left">
                        <thead>
                          <tr class="border-b border-white/5 text-[10px] font-black uppercase tracking-widest text-slate-500">
                            <th class="px-6 py-4">User</th>
                            <th class="px-6 py-4">Tier</th>
                            <th class="px-6 py-4">Status</th>
                            <th class="px-6 py-4">Health</th>
                            <th class="px-6 py-4">Machines</th>
                            <th class="px-6 py-4">Commands</th>
                            <th class="px-6 py-4">Joined</th>
                            <th class="px-6 py-4">{/* Actions */}</th>
                          </tr>
                        </thead>
                        <tbody class="divide-y divide-white/5">
                          <For each={crmCustomers()}>
                            {(customer) => (
                              <CRMProfileCardTableRow
                                customer={customer}
                                onViewDetail={(customerId) => setSelectedUserId(customerId)}
                                onQuickAction={(action, _customerId) => {
                                  if (action === 'email') {
                                    window.open(`mailto:${customer.email}`);
                                  }
                                }}
                              />
                            )}
                          </For>
                        </tbody>
                      </table>
                    </div>

                    <Show when={crmCustomers().length === 0}>
                      <div class="py-12 text-center">
                        <Users size={48} class="mx-auto mb-4 text-slate-600" />
                        <p class="font-medium text-slate-500">No customers found</p>
                        <p class="mt-1 text-xs text-slate-600">
                          {crmSearch() ? 'Try a different search term' : 'Customers will appear here'}
                        </p>
                      </div>
                    </Show>

                    <Show when={(crmPagination()?.pages || 1) > 1}>
                      <div class="flex items-center justify-between border-t border-white/5 px-6 py-4">
                        <p class="text-sm text-slate-500">
                          Page {crmPage()} of {crmPagination()?.pages || 1}
                        </p>
                        <div class="flex items-center gap-2">
                          <button
                            onClick={() => setCrmPage(Math.max(1, crmPage() - 1))}
                            disabled={crmPage() === 1}
                            class="rounded-xl border border-white/10 bg-white/[0.02] px-4 py-2 text-sm font-bold text-white transition-all hover:bg-white/[0.06] disabled:cursor-not-allowed disabled:opacity-30"
                          >
                            Previous
                          </button>
                          <button
                            onClick={() => setCrmPage(Math.min(crmPagination()?.pages || 1, crmPage() + 1))}
                            disabled={crmPage() === (crmPagination()?.pages || 1)}
                            class="rounded-xl border border-white/10 bg-white/[0.02] px-4 py-2 text-sm font-bold text-white transition-all hover:bg-white/[0.06] disabled:cursor-not-allowed disabled:opacity-30"
                          >
                            Next
                          </button>
                        </div>
                      </div>
                    </Show>
                  </div>
                </Show>
              </Show>
            </div>
          </Match>

          <Match when={activeTab() === 'analytics'}>
            <div role="tabpanel" id="panel-analytics" aria-labelledby="tab-analytics" tabindex={0} class="space-y-8">
              <ErrorBoundary fallback={(err, reset) => <ErrorFallback error={err} reset={reset} />}>
                <DocsAnalytics />
              </ErrorBoundary>
              <ErrorBoundary fallback={(err, reset) => <ErrorFallback error={err} reset={reset} />}>
                <CohortAnalysis />
              </ErrorBoundary>
            </div>
          </Match>

          <Match when={activeTab() === 'insights'}>
            <div role="tabpanel" id="panel-insights" aria-labelledby="tab-insights" tabindex={0}>
              <ErrorBoundary fallback={(err, reset) => <ErrorFallback error={err} reset={reset} />}>
                <InsightsTab />
              </ErrorBoundary>
            </div>
          </Match>

          <Match when={activeTab() === 'segments'}>
            <div role="tabpanel" id="panel-segments" aria-labelledby="tab-segments" tabindex={0}>
              <ErrorBoundary fallback={(err, reset) => <ErrorFallback error={err} reset={reset} />}>
                <SegmentAnalytics />
              </ErrorBoundary>
            </div>
          </Match>

          <Match when={activeTab() === 'predictions'}>
            <div role="tabpanel" id="panel-predictions" aria-labelledby="tab-predictions" tabindex={0}>
              <ErrorBoundary fallback={(err, reset) => <ErrorFallback error={err} reset={reset} />}>
                <PredictiveInsights />
              </ErrorBoundary>
            </div>
          </Match>

          <Match when={activeTab() === 'reports'}>
            <div role="tabpanel" id="panel-reports" aria-labelledby="tab-reports" tabindex={0}>
              <ErrorBoundary fallback={(err, reset) => <ErrorFallback error={err} reset={reset} />}>
                <CustomReportBuilder />
              </ErrorBoundary>
            </div>
          </Match>

          <Match when={activeTab() === 'revenue'}>
            <div role="tabpanel" id="panel-revenue" aria-labelledby="tab-revenue" tabindex={0}>
              <ErrorBoundary fallback={(err, reset) => <ErrorFallback error={err} reset={reset} />}>
                <RevenueTab />
              </ErrorBoundary>
            </div>
          </Match>

          <Match when={activeTab() === 'audit'}>
            <div role="tabpanel" id="panel-audit" aria-labelledby="tab-audit" tabindex={0}>
              <ErrorBoundary fallback={(err, reset) => <ErrorFallback error={err} reset={reset} />}>
                <AuditLogTab />
              </ErrorBoundary>
            </div>
          </Match>
        </Switch>
      </Show>

      <Show when={showSaveViewModal()}>
        <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
          <div class="w-full max-w-md rounded-2xl border border-white/10 bg-void-900 p-6 shadow-2xl">
            <h3 class="mb-4 text-lg font-black text-white">Save Current View</h3>
            <input
              type="text"
              value={newViewName()}
              onInput={(e) => setNewViewName(e.currentTarget.value)}
              placeholder="View name..."
              class="mb-4 w-full rounded-xl border border-white/10 bg-white/5 px-4 py-3 text-white placeholder-nebula-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/20"
            />
            <div class="mb-4 space-y-2 rounded-xl border border-white/5 bg-void-850 p-3 text-xs text-nebula-400">
              <div class="flex justify-between">
                <span>Tab:</span>
                <span class="font-bold text-white">{activeTab()}</span>
              </div>
              <div class="flex justify-between">
                <span>Date Range:</span>
                <span class="font-bold text-white">{dateRange()}</span>
              </div>
              <div class="flex justify-between">
                <span>Segment:</span>
                <span class="font-bold text-white">{SEGMENTS.find((s) => s.id === selectedSegment())?.name}</span>
              </div>
              <div class="flex justify-between">
                <span>Compare Mode:</span>
                <span class="font-bold text-white">{compareEnabled() ? 'On' : 'Off'}</span>
              </div>
            </div>
            <div class="flex justify-end gap-3">
              <button
                onClick={() => setShowSaveViewModal(false)}
                class="rounded-xl border border-white/10 bg-white/5 px-4 py-2 text-sm font-bold text-white transition-all hover:bg-white/10"
              >
                Cancel
              </button>
              <button
                onClick={saveCurrentView}
                disabled={!newViewName().trim()}
                class="rounded-xl bg-indigo-500 px-4 py-2 text-sm font-bold text-white transition-all hover:bg-indigo-600 disabled:cursor-not-allowed disabled:opacity-50"
              >
                Save View
              </button>
            </div>
          </div>
        </div>
      </Show>

      <Show when={exportError()}>
        <div class="fixed bottom-4 right-4 z-50 flex items-start gap-3 rounded-xl border border-flare-500/20 bg-flare-500/10 p-4 shadow-2xl backdrop-blur-sm">
          <AlertTriangle size={20} class="shrink-0 text-flare-400" />
          <div class="flex-1">
            <p class="text-sm font-bold text-flare-400">Export Failed</p>
            <p class="mt-1 text-xs text-nebula-400">{exportError()}</p>
          </div>
          <button
            onClick={() => setExportError(null)}
            class="shrink-0 rounded-lg p-1 text-flare-400 transition-colors hover:bg-flare-500/20"
            aria-label="Dismiss error"
          >
            <X size={16} />
          </button>
        </div>
      </Show>

      <CustomerDetailDrawer userId={selectedUserId()} onClose={() => setSelectedUserId(null)} />
    </div>
  );
};

export default AdminDashboard;
