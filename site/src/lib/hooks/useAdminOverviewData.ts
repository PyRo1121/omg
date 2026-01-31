import { createMemo } from 'solid-js';
import {
  useAdminDashboard,
  useAdminFirehose,
  useAdminAdvancedMetrics,
  useSiteGeoAnalytics,
  useSiteRealtimeAnalytics,
} from '../api-hooks';
import {
  transformToExecutiveKPI,
  transformToAdvancedMetrics,
  transformFirehoseEvents,
  transformGeoDistribution,
} from '../transforms/admin';
import type { ExecutiveKPI, AdvancedMetrics, FirehoseEvent, GeoDistribution, CommandHealth } from '~/types';

export function useAdminOverviewData() {
  const dashboardQuery = useAdminDashboard();
  const firehoseQuery = useAdminFirehose(100);
  const advancedMetricsQuery = useAdminAdvancedMetrics();
  const siteGeoQuery = useSiteGeoAnalytics(30);
  const realtimeQuery = useSiteRealtimeAnalytics();

  const executiveKPI = createMemo<ExecutiveKPI>(() =>
    transformToExecutiveKPI(dashboardQuery.data, advancedMetricsQuery.data)
  );

  const advancedMetrics = createMemo<AdvancedMetrics | undefined>(() =>
    transformToAdvancedMetrics(advancedMetricsQuery.data)
  );

  const firehoseEvents = createMemo<FirehoseEvent[]>(() =>
    transformFirehoseEvents(firehoseQuery.data?.events || [])
  );

  const geoDistribution = createMemo<GeoDistribution[]>(() => {
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

  const commandHealth = createMemo<CommandHealth>(() => {
    const health = dashboardQuery.data?.overview?.command_health;
    const total = (health?.success || 0) + (health?.failure || 0);
    if (total === 0) return { success: 95, failure: 5 };
    return {
      success: ((health?.success || 0) / total) * 100,
      failure: ((health?.failure || 0) / total) * 100,
    };
  });

  const isLoading = () =>
    dashboardQuery.isLoading ||
    firehoseQuery.isLoading ||
    advancedMetricsQuery.isLoading;

  const refetchAll = () => {
    dashboardQuery.refetch();
    firehoseQuery.refetch();
    advancedMetricsQuery.refetch();
    siteGeoQuery.refetch();
    realtimeQuery.refetch();
  };

  return {
    executiveKPI,
    advancedMetrics,
    firehoseEvents,
    geoDistribution,
    commandHealth,
    isLoading,
    refetchAll,
    refetchFirehose: () => firehoseQuery.refetch(),
  };
}

function getCountryName(code: string): string {
  const countries: Record<string, string> = {
    US: 'United States', DE: 'Germany', GB: 'United Kingdom', FR: 'France',
    CA: 'Canada', JP: 'Japan', AU: 'Australia', BR: 'Brazil', IN: 'India',
    NL: 'Netherlands', SE: 'Sweden', ES: 'Spain', IT: 'Italy', KR: 'South Korea',
  };
  return countries[code] || code || 'Unknown';
}
