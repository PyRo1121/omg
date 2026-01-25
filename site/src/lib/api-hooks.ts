import { createQuery } from '@tanstack/solid-query';
import * as api from './api';
import { apiRequest } from './api';

// Reusable Query Hooks
export function useTeamData() {
  return createQuery(() => ({
    queryKey: ['team-data'],
    queryFn: () => api.getTeamMembers(),
  }));
}

export function useTeamPolicies() {
  return createQuery(() => ({
    queryKey: ['team-policies'],
    queryFn: () => api.getTeamPolicies(),
  }));
}

export function useNotificationSettings() {
  return createQuery(() => ({
    queryKey: ['notification-settings'],
    queryFn: () => api.getNotificationSettings(),
  }));
}

export function useTeamAuditLogs(params?: { limit?: number; offset?: number }) {
  return createQuery(() => ({
    queryKey: ['team-audit-logs', params],
    queryFn: () => api.getTeamAuditLogs(params),
  }));
}

export function useAdminEvents() {
  return createQuery(() => ({
    queryKey: ['admin-events'],
    queryFn: () => api.getAdminActivity(),
  }));
}

export function useFleetStatus() {
  return createQuery(() => ({
    queryKey: ['fleet-status'],
    queryFn: () => apiRequest<api.Machine[]>('/api/fleet/status'),
  }));
}

export function useTeamAnalytics() {
  return createQuery(() => ({
    queryKey: ['team-analytics'],
    queryFn: () => api.getAdminAnalytics(),
  }));
}

export function useDashboardData() {
  return createQuery(() => ({
    queryKey: ['dashboard'],
    queryFn: () => api.getDashboard(),
  }));
}

export function useAdminDashboard() {
  return createQuery(() => ({
    queryKey: ['admin-dashboard'],
    queryFn: () => api.getAdminDashboard(),
  }));
}

export function useAdminFirehose(limit = 50) {
  return createQuery(() => ({
    queryKey: ['admin-firehose', limit],
    queryFn: () => api.getAdminFirehose(limit),
    refetchInterval: 5000, // Real-time polling
  }));
}

export function useAdminAnalytics() {
  return createQuery(() => ({
    queryKey: ['admin-analytics'],
    queryFn: () => api.getAdminAnalytics(),
  }));
}
