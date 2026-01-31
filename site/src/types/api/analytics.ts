export interface SiteAnalyticsOverview {
  summary?: {
    total_pageviews: number;
    total_visitors: number;
    total_sessions: number;
  };
  top_pages?: Array<{
    path: string;
    views: number;
  }>;
  top_referrers?: Array<{
    referrer_domain: string | null;
    visitors: number;
  }>;
  device_breakdown?: Array<{
    device_type: string;
    visitors: number;
  }>;
}

export interface SiteGeoAnalytics {
  geo_distribution?: Array<{
    country_code: string;
    user_count: number;
    percentage: number;
  }>;
  total_countries?: number;
  total_engagement?: number;
}

export interface SiteRealtimeAnalytics {
  active_visitors?: number;
  top_pages?: Array<{
    page_path: string;
    count: number;
  }>;
}
