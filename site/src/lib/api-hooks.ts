import { createQuery, createMutation, useQueryClient } from '@tanstack/solid-query';
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
    queryFn: () =>
      apiRequest<{ members: api.Machine[] }>('/api/fleet/status').then(res => res.members),
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
    refetchInterval: 5000,
  }));
}

// Mutations
export function useRevokeMachine() {
  const queryClient = useQueryClient();
  return createMutation(() => ({
    mutationFn: (machineId: string) => api.revokeMachine(machineId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['team-data'] });
      queryClient.invalidateQueries({ queryKey: ['fleet-status'] });
      queryClient.invalidateQueries({ queryKey: ['dashboard'] });
    },
  }));
}

export function useCreatePolicy() {
  const queryClient = useQueryClient();
  return createMutation(() => ({
    mutationFn: (policy: { scope: string; rule: string; value: string; enforced?: boolean }) =>
      api.createTeamPolicy(policy),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['team-policies'] });
    },
  }));
}

export function useDeletePolicy() {
  const queryClient = useQueryClient();
  return createMutation(() => ({
    mutationFn: (id: string) => api.deleteTeamPolicy(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['team-policies'] });
    },
  }));
}

export function useUpdateThreshold() {
  const queryClient = useQueryClient();
  return createMutation(() => ({
    mutationFn: ({ type, value }: { type: string; value: number }) =>
      api.updateAlertThreshold(type, value),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['team-data'] });
    },
  }));
}

export function useUpdateNotifications() {
  const queryClient = useQueryClient();
  return createMutation(() => ({
    mutationFn: (settings: api.NotificationSetting[]) => api.updateNotificationSettings(settings),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['notification-settings'] });
    },
  }));
}

export function useAdminRevenue() {
  return createQuery(() => ({
    queryKey: ['admin-revenue'],
    queryFn: () => api.getAdminRevenue(),
  }));
}

export function useAdminAnalytics() {
  return createQuery(() => ({
    queryKey: ['admin-analytics'],
    queryFn: () => api.getAdminAnalytics(),
  }));
}

export function useAdminAuditLog(page = 1, limit = 50, action = '') {
  return createQuery(() => ({
    queryKey: ['admin-audit-log', page, limit, action],
    queryFn: () => api.getAdminAuditLog(page, limit, action),
  }));
}

export function useAdminCRMUsers(page = 1, limit = 50, search = '') {
  return createQuery(() => ({
    queryKey: ['admin-crm-users', page, limit, search],
    queryFn: () => api.getAdminUsers(page, limit, search),
  }));
}

export function useAdminUserDetail(userId: string) {
  return createQuery(() => ({
    queryKey: ['admin-user-detail', userId],
    queryFn: () => api.getAdminUserDetail(userId),
    enabled: !!userId,
  }));
}

export function useAdminCohorts() {
  return createQuery(() => ({
    queryKey: ['admin-cohorts'],
    queryFn: () => api.getAdminCohorts(),
  }));
}
