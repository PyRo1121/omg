import { createSignal, createEffect, onCleanup, Accessor } from 'solid-js';

// ============================================================================
// Types
// ============================================================================

export interface CommandEvent {
  id: string;
  license_key: string;
  license_tier: 'free' | 'pro' | 'team' | 'enterprise';
  user_email?: string;
  command: string;
  package_name?: string;
  duration_ms: number;
  status: 'success' | 'error';
  error_message?: string;
  platform: string;
  version: string;
  hostname?: string;
  machine_id: string;
  geo?: {
    country_code: string;
    country: string;
    region?: string;
    city?: string;
    latitude?: number;
    longitude?: number;
  };
  timestamp: string;
}

export interface SessionEvent {
  session_id: string;
  license_key: string;
  license_tier: 'free' | 'pro' | 'team' | 'enterprise';
  machine_id: string;
  hostname?: string;
  platform: string;
  version: string;
  geo?: {
    country_code: string;
    country: string;
    region?: string;
    city?: string;
    latitude?: number;
    longitude?: number;
  };
  started_at: string;
  last_activity_at: string;
  command_count: number;
  is_active: boolean;
}

export interface HealthUpdate {
  overall_score: number;
  engagement_score: number;
  adoption_score: number;
  satisfaction_score: number;
  previous_score?: number;
  trend: 'up' | 'down' | 'stable';
  updated_at: string;
}

export type TelemetryEventType = 'command_event' | 'session_start' | 'session_end' | 'health_update';

export interface TelemetryMessage {
  type: TelemetryEventType;
  data: CommandEvent | SessionEvent | HealthUpdate;
  timestamp: string;
}

export interface RealtimeConnectionState {
  status: 'connecting' | 'connected' | 'disconnected' | 'error';
  error?: string;
  reconnectAttempt: number;
  lastConnectedAt?: string;
}

export interface RealtimeDataState {
  commands: CommandEvent[];
  activeSessions: Map<string, SessionEvent>;
  health: HealthUpdate | null;
}

export interface UseRealtimeDataOptions {
  wsUrl: string;
  maxCommands?: number;
  autoReconnect?: boolean;
  maxReconnectAttempts?: number;
  onCommand?: (event: CommandEvent) => void;
  onSession?: (event: SessionEvent) => void;
  onHealth?: (update: HealthUpdate) => void;
  onError?: (error: Error) => void;
}

export interface UseRealtimeDataReturn {
  // Connection state
  connectionState: Accessor<RealtimeConnectionState>;
  isConnected: Accessor<boolean>;

  // Data accessors
  commands: Accessor<CommandEvent[]>;
  activeSessions: Accessor<SessionEvent[]>;
  activeSessionsMap: Accessor<Map<string, SessionEvent>>;
  health: Accessor<HealthUpdate | null>;

  // Computed values
  commandCount: Accessor<number>;
  activeSessionCount: Accessor<number>;
  sessionsByCountry: Accessor<Map<string, SessionEvent[]>>;

  // Actions
  connect: () => void;
  disconnect: () => void;
  clearCommands: () => void;
}

// ============================================================================
// Constants
// ============================================================================

const DEFAULT_MAX_COMMANDS = 100;
const DEFAULT_MAX_RECONNECT_ATTEMPTS = 10;
const BASE_RECONNECT_DELAY = 1000; // 1 second
const MAX_RECONNECT_DELAY = 30000; // 30 seconds

// ============================================================================
// Hook Implementation
// ============================================================================

export function useRealtimeData(options: UseRealtimeDataOptions): UseRealtimeDataReturn {
  const {
    wsUrl,
    maxCommands = DEFAULT_MAX_COMMANDS,
    autoReconnect = true,
    maxReconnectAttempts = DEFAULT_MAX_RECONNECT_ATTEMPTS,
    onCommand,
    onSession,
    onHealth,
    onError,
  } = options;

  // Connection state
  const [connectionState, setConnectionState] = createSignal<RealtimeConnectionState>({
    status: 'disconnected',
    reconnectAttempt: 0,
  });

  // Data state
  const [commands, setCommands] = createSignal<CommandEvent[]>([]);
  const [activeSessions, setActiveSessions] = createSignal<Map<string, SessionEvent>>(new Map());
  const [health, setHealth] = createSignal<HealthUpdate | null>(null);

  // Internal refs
  let ws: WebSocket | null = null;
  let reconnectTimeoutId: ReturnType<typeof setTimeout> | null = null;
  let isManualDisconnect = false;

  // Computed values
  const isConnected = () => connectionState().status === 'connected';
  const commandCount = () => commands().length;
  const activeSessionCount = () => activeSessions().size;

  const activeSessionsList = () => Array.from(activeSessions().values());

  const sessionsByCountry = () => {
    const map = new Map<string, SessionEvent[]>();
    for (const session of activeSessions().values()) {
      const countryCode = session.geo?.country_code || 'UNKNOWN';
      const existing = map.get(countryCode) || [];
      existing.push(session);
      map.set(countryCode, existing);
    }
    return map;
  };

  // Calculate exponential backoff delay
  const getReconnectDelay = (attempt: number): number => {
    const delay = Math.min(
      BASE_RECONNECT_DELAY * Math.pow(2, attempt),
      MAX_RECONNECT_DELAY
    );
    // Add jitter (0-25% of delay)
    const jitter = delay * Math.random() * 0.25;
    return delay + jitter;
  };

  // Handle incoming telemetry message
  const handleMessage = (message: TelemetryMessage): void => {
    switch (message.type) {
      case 'command_event': {
        const event = message.data as CommandEvent;
        setCommands((prev) => {
          const updated = [event, ...prev];
          // Keep only the latest N commands
          return updated.slice(0, maxCommands);
        });
        onCommand?.(event);
        break;
      }

      case 'session_start': {
        const session = message.data as SessionEvent;
        setActiveSessions((prev) => {
          const updated = new Map(prev);
          updated.set(session.session_id, { ...session, is_active: true });
          return updated;
        });
        onSession?.(session);
        break;
      }

      case 'session_end': {
        const session = message.data as SessionEvent;
        setActiveSessions((prev) => {
          const updated = new Map(prev);
          updated.delete(session.session_id);
          return updated;
        });
        onSession?.({ ...session, is_active: false });
        break;
      }

      case 'health_update': {
        const update = message.data as HealthUpdate;
        setHealth(update);
        onHealth?.(update);
        break;
      }
    }
  };

  // Connect to WebSocket
  const connect = (): void => {
    if (ws?.readyState === WebSocket.OPEN || ws?.readyState === WebSocket.CONNECTING) {
      return;
    }

    isManualDisconnect = false;
    setConnectionState((prev) => ({
      ...prev,
      status: 'connecting',
    }));

    try {
      ws = new WebSocket(wsUrl);

      ws.onopen = () => {
        setConnectionState({
          status: 'connected',
          reconnectAttempt: 0,
          lastConnectedAt: new Date().toISOString(),
        });
      };

      ws.onmessage = (event) => {
        try {
          const message = JSON.parse(event.data) as TelemetryMessage;
          handleMessage(message);
        } catch (err) {
          console.error('[useRealtimeData] Failed to parse message:', err);
        }
      };

      ws.onerror = (event) => {
        const error = new Error('WebSocket connection error');
        setConnectionState((prev) => ({
          ...prev,
          status: 'error',
          error: error.message,
        }));
        onError?.(error);
      };

      ws.onclose = (event) => {
        ws = null;

        if (isManualDisconnect) {
          setConnectionState({
            status: 'disconnected',
            reconnectAttempt: 0,
          });
          return;
        }

        const currentState = connectionState();
        const nextAttempt = currentState.reconnectAttempt + 1;

        if (autoReconnect && nextAttempt <= maxReconnectAttempts) {
          const delay = getReconnectDelay(nextAttempt - 1);

          setConnectionState({
            status: 'disconnected',
            reconnectAttempt: nextAttempt,
            error: `Connection closed. Reconnecting in ${Math.round(delay / 1000)}s...`,
          });

          reconnectTimeoutId = setTimeout(() => {
            connect();
          }, delay);
        } else {
          setConnectionState({
            status: 'error',
            reconnectAttempt: nextAttempt,
            error: autoReconnect
              ? 'Max reconnection attempts reached'
              : 'Connection closed',
          });
        }
      };
    } catch (err) {
      const error = err instanceof Error ? err : new Error('Failed to connect');
      setConnectionState({
        status: 'error',
        reconnectAttempt: connectionState().reconnectAttempt,
        error: error.message,
      });
      onError?.(error);
    }
  };

  // Disconnect from WebSocket
  const disconnect = (): void => {
    isManualDisconnect = true;

    if (reconnectTimeoutId) {
      clearTimeout(reconnectTimeoutId);
      reconnectTimeoutId = null;
    }

    if (ws) {
      ws.close(1000, 'Client disconnect');
      ws = null;
    }

    setConnectionState({
      status: 'disconnected',
      reconnectAttempt: 0,
    });
  };

  // Clear commands buffer
  const clearCommands = (): void => {
    setCommands([]);
  };

  // Cleanup on unmount
  onCleanup(() => {
    disconnect();
  });

  return {
    // Connection state
    connectionState,
    isConnected,

    // Data accessors
    commands,
    activeSessions: activeSessionsList,
    activeSessionsMap: activeSessions,
    health,

    // Computed values
    commandCount,
    activeSessionCount,
    sessionsByCountry,

    // Actions
    connect,
    disconnect,
    clearCommands,
  };
}

export default useRealtimeData;
