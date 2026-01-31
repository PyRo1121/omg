export type DateRange = '7d' | '30d' | '90d' | 'custom';

export type AdminTab = 'overview' | 'crm' | 'analytics' | 'insights' | 'revenue' | 'audit' | 'segments' | 'predictions';

export interface SavedView {
  id: string;
  name: string;
  tab: AdminTab;
  dateRange: DateRange;
  segment: string;
  compareEnabled: boolean;
}

export interface DashboardFilters {
  dateRange: DateRange;
  segment: string;
  compareEnabled: boolean;
}
