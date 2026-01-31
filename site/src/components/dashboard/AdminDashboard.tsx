import { Component, createSignal, createMemo, For, Show, Switch, Match } from 'solid-js';
import {
  Activity,
  Users,
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
} from 'lucide-solid';
import * as api from '../../lib/api';
import {
  useAdminDashboard,
  useAdminFirehose,
  useAdminCRMUsers,
  useAdminAdvancedMetrics,
  useSiteGeoAnalytics,
  useSiteRealtimeAnalytics,
  useSiteAnalyticsOverview,
} from '../../lib/api-hooks';
import { CardSkeleton } from '../ui/Skeleton';
import { RevenueTab } from './admin/RevenueTab';
import { AuditLogTab } from './admin/AuditLogTab';
import { CustomerDetailDrawer } from './admin/CustomerDetailDrawer';
import { InsightsTab } from './admin/insights/InsightsTab';
import { SegmentAnalytics } from './admin/SegmentAnalytics';
import { PredictiveInsights } from './admin/PredictiveInsights';
import { OverviewTab } from './admin/tabs/OverviewTab';
import { CRMTab } from './admin/tabs/CRMTab';
import { AnalyticsTab } from './admin/tabs/AnalyticsTab';
import { TabErrorBoundary } from './admin/shared/TabErrorBoundary';
import ErrorCard from './admin/shared/ErrorCard';
import { createDashboardStore } from '../../lib/stores/dashboardStore';

type DateRange = '7d' | '30d' | '90d' | 'custom';
type SavedView = {
  id: string;
  name: string;
  tab: AdminTab;
  dateRange: DateRange;
  segment: string;
  compareEnabled: boolean;
};


import type {
  ExecutiveKPI,
  AdvancedMetrics,
  FirehoseEvent,
  GeoDistribution,
  CommandHealth,
  CRMCustomer,
  CustomerHealth,
} from './premium/types';

type AdminTab = 'overview' | 'crm' | 'analytics' | 'insights' | 'revenue' | 'audit' | 'segments' | 'predictions';

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
  const [store, actions] = createDashboardStore();
  const dashboardQuery = useAdminDashboard();
  const firehoseQuery = useAdminFirehose(100);
  const crmUsersQuery = useAdminCRMUsers(store.crm.page, 25, store.crm.search);
  const advancedMetricsQuery = useAdminAdvancedMetrics();
  const siteGeoQuery = useSiteGeoAnalytics(store.filters.dateRange === '7d' ? 7 : store.filters.dateRange === '90d' ? 90 : 30);
  const realtimeQuery = useSiteRealtimeAnalytics();
  const siteOverviewQuery = useSiteAnalyticsOverview(store.filters.dateRange === '7d' ? 7 : store.filters.dateRange === '90d' ? 90 : 30);

  const executiveKPI = createMemo(() =>
    transformToExecutiveKPI(dashboardQuery.data, advancedMetricsQuery.data)
  );

  const advancedMetrics = createMemo(() =>
    transformToAdvancedMetrics(advancedMetricsQuery.data)
  );

  const firehoseEvents = createMemo(() =>
    transformFirehoseEvents(firehoseQuery.data?.events || [])
  );

  const geoDistribution = createMemo(() => {
    const geoData = siteGeoQuery.data?.geo_distribution || [];
    if (geoData.length > 0) {
      return geoData.map(g => ({
        country: getCountryName(g.country_code),
        country_code: g.country_code,
        count: g.user_count,
        percentage: g.percentage,
      }));
    }
    return transformGeoDistribution(dashboardQuery.data?.geo_distribution || []);
  });

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

  // Export handlers
  const handleExport = async (type: 'users' | 'usage' | 'audit') => {
    actions.setExporting(true);
    actions.closeExportMenu();
    try {
      let data: string;
      let filename: string;
      switch (type) {
        case 'users':
          data = await api.exportAdminUsers();
          filename = `omg-users-${new Date().toISOString().split('T')[0]}.csv`;
          break;
        case 'usage':
          data = await api.exportAdminUsage(30);
          filename = `omg-usage-${new Date().toISOString().split('T')[0]}.csv`;
          break;
        case 'audit':
          data = await api.exportAdminAudit(30);
          filename = `omg-audit-${new Date().toISOString().split('T')[0]}.csv`;
          break;
      }
      api.downloadCSV(data, filename);
    } catch (error) {
      console.error('Export failed:', error);
    } finally {
      actions.setExporting(false);
    }
  };

  const saveCurrentView = () => {
    actions.saveView();
  };

  const loadView = (view: SavedView) => {
    actions.loadView(view);
  };

  const tabCounts = createMemo(() => ({
    crm: crmUsersQuery.data?.pagination?.total || 0,
    insights: advancedMetricsQuery.data?.expansion_opportunities?.length || 0,
    predictions:
      (advancedMetricsQuery.data?.churn_risk_segments?.filter(
        (s) => s.risk_segment === 'high' || s.risk_segment === 'critical'
      ).length || 0) + (advancedMetricsQuery.data?.expansion_opportunities?.length || 0),
  }));

  const TABS_ORDER: AdminTab[] = ['overview', 'crm', 'analytics', 'insights', 'segments', 'predictions', 'revenue', 'audit'];

  const handleTabKeyDown = (e: KeyboardEvent, tabId: AdminTab) => {
    const currentIndex = TABS_ORDER.indexOf(tabId);
    let nextIndex = currentIndex;

    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      e.preventDefault();
      nextIndex = (currentIndex + 1) % TABS_ORDER.length;
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      e.preventDefault();
      nextIndex = (currentIndex - 1 + TABS_ORDER.length) % TABS_ORDER.length;
    } else if (e.key === 'Home') {
      e.preventDefault();
      nextIndex = 0;
    } else if (e.key === 'End') {
      e.preventDefault();
      nextIndex = TABS_ORDER.length - 1;
    } else {
      return;
    }

    const nextTab = TABS_ORDER[nextIndex];
    actions.setTab(nextTab);
    
    setTimeout(() => {
      const nextButton = document.querySelector(`[role="tab"][aria-controls="tabpanel-${nextTab}"]`) as HTMLElement;
      nextButton?.focus();
    }, 0);
  };

  const TabButton = (props: { id: AdminTab; icon: Component<{ size?: number }>; label: string; count?: number }) => {
    const isActive = () => store.navigation.activeTab === props.id;

    return (
      <button
        role="tab"
        aria-selected={isActive()}
        aria-controls={`tabpanel-${props.id}`}
        tabindex={isActive() ? 0 : -1}
        onClick={() => actions.setTab(props.id)}
        onKeyDown={(e) => handleTabKeyDown(e, props.id)}
        class={`relative flex items-center gap-2 rounded-xl px-4 py-2.5 font-bold transition-all duration-300 ${
          isActive()
            ? 'bg-gradient-to-r from-electric-500/20 to-photon-500/20 text-white shadow-lg shadow-electric-500/10 ring-1 ring-electric-500/30'
            : 'text-nebula-400 hover:bg-white/5 hover:text-white'
        }`}
      >
        <Show when={isActive()}>
          <div class="absolute inset-0 rounded-xl bg-gradient-to-r from-electric-500/10 to-photon-500/10 blur-sm" />
        </Show>
        <span class="relative"><props.icon size={16} /></span>
        <span class="relative">{props.label}</span>
        <Show when={props.count !== undefined && props.count > 0}>
          <span
            class={`relative rounded-full px-1.5 py-0.5 text-2xs font-black ${
              isActive()
                ? 'bg-electric-500/20 text-electric-400' 
                : 'bg-white/10 text-nebula-300'
            }`}
          >
            {props.count}
          </span>
        </Show>
      </button>
    );
  };

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
              value={store.filters.dateRange}
              onChange={(e) => actions.setDateRange(e.currentTarget.value as DateRange)}
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
              value={store.filters.segment}
              onChange={(e) => actions.setSegment(e.currentTarget.value)}
              class="bg-transparent text-sm font-bold text-white focus:outline-none"
            >
              <For each={SEGMENTS}>{(seg) => <option value={seg.id}>{seg.name}</option>}</For>
            </select>
          </div>

          <button
            onClick={() => actions.toggleCompare()}
            class={`flex items-center gap-2 rounded-xl border px-3 py-2 text-xs font-bold transition-all sm:text-sm ${
              store.filters.compareEnabled
                ? 'border-indigo-500/50 bg-indigo-500/10 text-indigo-400'
                : 'border-white/10 bg-white/[0.03] text-white hover:bg-white/[0.06]'
            }`}
          >
            <GitCompare size={14} />
            <span class="hidden sm:inline">Compare</span>
          </button>

          <button
            onClick={() => actions.showSaveViewModal()}
            class="flex items-center gap-2 rounded-xl border border-white/10 bg-white/[0.03] px-3 py-2 text-xs font-bold text-white transition-all hover:bg-white/[0.06] sm:text-sm"
          >
            <Save size={14} />
            <span class="hidden sm:inline">Save View</span>
          </button>

          <div class="relative">
            <button
              onClick={(e) => {
                e.stopPropagation();
                actions.toggleExportMenu();
              }}
              disabled={store.ui.isExporting}
              class="flex items-center gap-2 rounded-xl border border-white/10 bg-white/[0.03] px-3 py-2 text-xs font-bold text-white transition-all hover:bg-white/[0.06] disabled:cursor-not-allowed disabled:opacity-50 sm:text-sm"
            >
              <Download size={14} />
              <span class="hidden sm:inline">Export</span>
              <ChevronDown size={12} class={`transition-transform ${store.ui.exportMenuOpen ? 'rotate-180' : ''}`} />
            </button>

            <Show when={store.ui.exportMenuOpen}>
              <div class="absolute right-0 top-full z-50 mt-2 w-56 origin-top-right rounded-xl border border-white/10 bg-[#0d0d0e] p-1 shadow-2xl sm:right-0 max-sm:right-0 max-sm:left-auto">
                <button
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
                  onClick={() => handleExport('usage')}
                  class="flex w-full items-center gap-3 rounded-lg px-4 py-2.5 text-left text-sm text-white transition-colors hover:bg-white/5"
                >
                  <BarChart3 size={16} class="text-cyan-400" />
                  <div>
                    <div class="font-medium">Usage ({store.filters.dateRange})</div>
                    <div class="text-xs text-slate-500">Export usage data as CSV</div>
                  </div>
                </button>
                <button
                  onClick={() => handleExport('audit')}
                  class="flex w-full items-center gap-3 rounded-lg px-4 py-2.5 text-left text-sm text-white transition-colors hover:bg-white/5"
                >
                  <History size={16} class="text-purple-400" />
                  <div>
                    <div class="font-medium">Audit Log ({store.filters.dateRange})</div>
                    <div class="text-xs text-slate-500">Export audit log as CSV</div>
                  </div>
                </button>
              </div>
            </Show>
          </div>
        </div>
      </div>

      <Show when={store.filters.compareEnabled}>
        <div class="flex items-center gap-3 rounded-xl border border-indigo-500/30 bg-indigo-500/10 px-4 py-3">
          <GitCompare size={18} class="text-indigo-400" />
          <span class="text-sm font-medium text-indigo-300">
            Comparing current period with previous {store.filters.dateRange === '7d' ? '7 days' : store.filters.dateRange === '30d' ? '30 days' : '90 days'}
          </span>
          <button
            onClick={() => actions.toggleCompare()}
            class="ml-auto rounded-lg bg-indigo-500/20 px-3 py-1 text-xs font-bold text-indigo-300 hover:bg-indigo-500/30"
          >
            Exit Comparison
          </button>
        </div>
      </Show>

      <Show when={store.views.saved.length > 0}>
        <div class="flex items-center gap-2 overflow-x-auto">
          <span class="text-xs font-bold text-nebula-500">Saved Views:</span>
          <For each={store.views.saved}>
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

      <div 
        role="tablist" 
        aria-label="Dashboard sections"
        class="no-scrollbar flex items-center gap-1.5 overflow-x-auto rounded-2xl border border-white/5 bg-white/[0.02] p-1.5"
      >
        <TabButton id="overview" icon={Activity} label="Overview" />
        <TabButton id="crm" icon={Users} label="CRM" count={tabCounts().crm} />
        <TabButton id="analytics" icon={BarChart3} label="Analytics" />
        <TabButton id="insights" icon={Lightbulb} label="Insights" count={tabCounts().insights} />
        <TabButton id="segments" icon={Layers} label="Segments" />
        <TabButton id="predictions" icon={Brain} label="Predictions" count={tabCounts().predictions} />
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

      <Show 
        when={!dashboardQuery.isError} 
        fallback={
          <ErrorCard 
            title="Failed to Load Dashboard"
            message="Unable to fetch dashboard data. Please check your connection and try again."
            onRetry={() => dashboardQuery.refetch()}
          />
        }
      >
        <Show when={dashboardQuery.isSuccess}>
          <Switch>
            <Match when={store.navigation.activeTab === 'overview'}>
              <div role="tabpanel" id="tabpanel-overview" aria-labelledby="tab-overview">
                <TabErrorBoundary tab="Overview">
                  <OverviewTab
                executiveKPI={executiveKPI()}
                advancedMetrics={advancedMetrics()}
                firehoseEvents={firehoseEvents()}
                geoDistribution={geoDistribution()}
                commandHealth={commandHealth()}
                isMetricsLoading={advancedMetricsQuery.isLoading}
                onRefresh={() => firehoseQuery.refetch()}
              />
            </TabErrorBoundary>
              </div>
          </Match>

          <Match when={store.navigation.activeTab === 'crm'}>
            <div role="tabpanel" id="tabpanel-crm" aria-labelledby="tab-crm">
              <TabErrorBoundary tab="CRM">
              <CRMTab
                customers={crmCustomers()}
                pagination={crmPagination()}
                isLoading={crmUsersQuery.isLoading}
                isSuccess={crmUsersQuery.isSuccess}
                isError={crmUsersQuery.isError}
                onSearchChange={(search) => {
                  actions.setCRMSearch(search);
                }}
                onPageChange={actions.setCRMPage}
                onViewDetail={actions.setSelectedUserId}
                onRetry={() => crmUsersQuery.refetch()}
              />
            </TabErrorBoundary>
            </div>
          </Match>

          <Match when={store.navigation.activeTab === 'analytics'}>
            <div role="tabpanel" id="tabpanel-analytics" aria-labelledby="tab-analytics">
              <TabErrorBoundary tab="Analytics">
              <AnalyticsTab
                dateRange={store.filters.dateRange}
                siteOverview={siteOverviewQuery.data}
                siteGeo={siteGeoQuery.data}
                realtimeData={realtimeQuery.data}
                isOverviewLoading={siteOverviewQuery.isLoading}
                isRealtimeLoading={realtimeQuery.isLoading}
                isOverviewSuccess={siteOverviewQuery.isSuccess}
                isRealtimeSuccess={realtimeQuery.isSuccess}
                isOverviewError={siteOverviewQuery.isError}
                isRealtimeError={realtimeQuery.isError}
                onRetryOverview={() => siteOverviewQuery.refetch()}
                onRetryRealtime={() => realtimeQuery.refetch()}
              />
            </TabErrorBoundary>
            </div>
          </Match>

          <Match when={store.navigation.activeTab === 'insights'}>
            <div role="tabpanel" id="tabpanel-insights" aria-labelledby="tab-insights">
              <TabErrorBoundary tab="Insights">
                <InsightsTab />
              </TabErrorBoundary>
            </div>
          </Match>

          <Match when={store.navigation.activeTab === 'segments'}>
            <div role="tabpanel" id="tabpanel-segments" aria-labelledby="tab-segments">
              <TabErrorBoundary tab="Segments">
                <SegmentAnalytics />
              </TabErrorBoundary>
            </div>
          </Match>

          <Match when={store.navigation.activeTab === 'predictions'}>
            <div role="tabpanel" id="tabpanel-predictions" aria-labelledby="tab-predictions">
              <TabErrorBoundary tab="Predictions">
                <PredictiveInsights />
              </TabErrorBoundary>
            </div>
          </Match>

          <Match when={store.navigation.activeTab === 'revenue'}>
            <div role="tabpanel" id="tabpanel-revenue" aria-labelledby="tab-revenue">
              <TabErrorBoundary tab="Revenue">
                <RevenueTab />
              </TabErrorBoundary>
            </div>
          </Match>

          <Match when={store.navigation.activeTab === 'audit'}>
            <div role="tabpanel" id="tabpanel-audit" aria-labelledby="tab-audit">
              <TabErrorBoundary tab="Audit">
                <AuditLogTab />
              </TabErrorBoundary>
            </div>
          </Match>
          </Switch>
        </Show>
      </Show>

      <Show when={store.views.showSaveModal}>
        <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
          <div class="w-full max-w-md rounded-2xl border border-white/10 bg-void-900 p-6 shadow-2xl">
            <h3 class="mb-4 text-lg font-black text-white">Save Current View</h3>
            <input
              type="text"
              value={store.views.newViewName}
              onInput={(e) => actions.setNewViewName(e.currentTarget.value)}
              placeholder="View name..."
              class="mb-4 w-full rounded-xl border border-white/10 bg-white/5 px-4 py-3 text-white placeholder-nebula-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/20"
            />
            <div class="mb-4 space-y-2 rounded-xl border border-white/5 bg-void-850 p-3 text-xs text-nebula-400">
              <div class="flex justify-between">
                <span>Tab:</span>
                <span class="font-bold text-white">{store.navigation.activeTab}</span>
              </div>
              <div class="flex justify-between">
                <span>Date Range:</span>
                <span class="font-bold text-white">{store.filters.dateRange}</span>
              </div>
              <div class="flex justify-between">
                <span>Segment:</span>
                <span class="font-bold text-white">{SEGMENTS.find((s) => s.id === store.filters.segment)?.name}</span>
              </div>
              <div class="flex justify-between">
                <span>Compare Mode:</span>
                <span class="font-bold text-white">{store.filters.compareEnabled ? 'On' : 'Off'}</span>
              </div>
            </div>
            <div class="flex justify-end gap-3">
              <button
                onClick={() => actions.hideSaveViewModal()}
                class="rounded-xl border border-white/10 bg-white/5 px-4 py-2 text-sm font-bold text-white transition-all hover:bg-white/10"
              >
                Cancel
              </button>
              <button
                onClick={saveCurrentView}
                disabled={!store.views.newViewName.trim()}
                class="rounded-xl bg-indigo-500 px-4 py-2 text-sm font-bold text-white transition-all hover:bg-indigo-600 disabled:cursor-not-allowed disabled:opacity-50"
              >
                Save View
              </button>
            </div>
          </div>
        </div>
      </Show>

      <CustomerDetailDrawer userId={store.crm.selectedUserId} onClose={() => actions.setSelectedUserId(null)} />
    </div>
  );
};

export default AdminDashboard;
