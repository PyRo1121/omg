import { createQuery } from '@tanstack/solid-query';
import { apiRequest } from './api';

// Types for the new endpoints
export interface TeamAnalytics {
  total_commands: number;
  active_users: number;
  time_saved_ms: number;
  efficiency_score: number;
}

export interface FleetStatus {
  total_machines: number;
  online_machines: number;
  version_distribution: Record<string, number>;
  alerts: number;
}

export interface AdminEvent {
  id: string;
  type: string;
  description: string;
  user_email: string;
  timestamp: string;
}

// Reusable Query Hooks
export function useTeamAnalytics() {
  return createQuery(() => ({
    queryKey: ['team-analytics'],
    queryFn: () => apiRequest<TeamAnalytics>('/api/team/analytics'),
  }));
}

export function useFleetStatus() {
  return createQuery(() => ({
    queryKey: ['fleet-status'],
    queryFn: () => apiRequest<FleetStatus>('/api/fleet/status'),
  }));
}

export function useAdminEvents() {
  return createQuery(() => ({
    queryKey: ['admin-events'],
    queryFn: () => apiRequest<AdminEvent[]>('/api/admin/events'),
  }));
}
