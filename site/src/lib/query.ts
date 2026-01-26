import { QueryClient } from '@tanstack/solid-query';

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 60000,
      gcTime: 5 * 60 * 1000,
      retry: 2,
      retryDelay: (attemptIndex) => Math.min(1000 * 2 ** attemptIndex, 30000),
      networkMode: 'offlineFirst',
      useErrorBoundary: (error: any) => error.response?.status >= 500,
      refetchOnWindowFocus: false,
      refetchOnReconnect: true,
    },
    mutations: {
      retry: (failureCount, error: any) => {
        if (error.response?.status && error.response.status < 500) return false;
        return failureCount < 2;
      },
      useErrorBoundary: (error: any) => error.response?.status >= 500,
    },
  },
});
