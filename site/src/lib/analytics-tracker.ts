const API_URL = 'https://api.pyro1121.com/api/site/analytics/track';
const BATCH_INTERVAL = 5000;
const SESSION_TIMEOUT = 30 * 60 * 1000;

let eventQueue: AnalyticsEvent[] = [];
let sessionId: string | null = null;
let lastActivity = Date.now();
let flushTimeout: ReturnType<typeof setTimeout> | null = null;

interface AnalyticsEvent {
  event_type: 'pageview' | 'click' | 'form' | 'error' | 'performance';
  event_name: string;
  properties: Record<string, unknown>;
  timestamp: number;
  session_id: string;
  duration_ms?: number;
}

function generateSessionId(): string {
  return 'ses_' + Math.random().toString(36).substring(2) + Date.now().toString(36);
}

function getSessionId(): string {
  const now = Date.now();
  if (!sessionId || now - lastActivity > SESSION_TIMEOUT) {
    sessionId = generateSessionId();
  }
  lastActivity = now;
  return sessionId;
}

function getUtmParams(): Record<string, string> {
  if (typeof window === 'undefined') return {};
  const params = new URLSearchParams(window.location.search);
  const utm: Record<string, string> = {};
  ['utm_source', 'utm_medium', 'utm_campaign', 'utm_content', 'utm_term'].forEach(key => {
    const value = params.get(key);
    if (value) utm[key.replace('utm_', '')] = value;
  });
  return utm;
}

function getReferrer(): string {
  if (typeof document === 'undefined') return 'direct';
  const ref = document.referrer;
  if (!ref) return 'direct';
  try {
    const url = new URL(ref);
    if (url.hostname === window.location.hostname) return 'internal';
    return url.hostname;
  } catch {
    return 'direct';
  }
}

async function flushEvents(): Promise<void> {
  if (eventQueue.length === 0) return;

  const events = [...eventQueue];
  eventQueue = [];

  try {
    const response = await fetch(API_URL, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ events }),
      keepalive: true,
    });
    
    if (!response.ok) {
      eventQueue = [...events, ...eventQueue];
    }
  } catch {
    eventQueue = [...events, ...eventQueue];
  }
}

function scheduleFlush(): void {
  if (flushTimeout) return;
  flushTimeout = setTimeout(() => {
    flushTimeout = null;
    flushEvents();
  }, BATCH_INTERVAL);
}

function queueEvent(event: Omit<AnalyticsEvent, 'timestamp' | 'session_id'>): void {
  eventQueue.push({
    ...event,
    timestamp: Date.now(),
    session_id: getSessionId(),
  });
  scheduleFlush();
}

export function trackPageview(path?: string): void {
  if (typeof window === 'undefined') return;
  
  queueEvent({
    event_type: 'pageview',
    event_name: 'page_view',
    properties: {
      path: path || window.location.pathname,
      url: window.location.href,
      referrer: getReferrer(),
      utm: getUtmParams(),
      viewport: {
        width: window.innerWidth,
        height: window.innerHeight,
      },
      screen: {
        width: screen.width,
        height: screen.height,
      },
    },
  });
}

export function trackClick(target: string, metadata?: Record<string, unknown>): void {
  queueEvent({
    event_type: 'click',
    event_name: 'element_click',
    properties: {
      target,
      path: typeof window !== 'undefined' ? window.location.pathname : '',
      ...metadata,
    },
  });
}

export function trackFormSubmit(formId: string, success: boolean): void {
  queueEvent({
    event_type: 'form',
    event_name: success ? 'form_submit_success' : 'form_submit_error',
    properties: {
      form_id: formId,
      path: typeof window !== 'undefined' ? window.location.pathname : '',
    },
  });
}

export function trackError(error: string, context?: Record<string, unknown>): void {
  queueEvent({
    event_type: 'error',
    event_name: 'client_error',
    properties: {
      error,
      path: typeof window !== 'undefined' ? window.location.pathname : '',
      ...context,
    },
  });
}

export function trackPerformance(metrics: {
  lcp?: number;
  fid?: number;
  cls?: number;
  ttfb?: number;
}): void {
  queueEvent({
    event_type: 'performance',
    event_name: 'web_vitals',
    properties: {
      path: typeof window !== 'undefined' ? window.location.pathname : '',
      ...metrics,
    },
  });
}

export function initAnalytics(): void {
  if (typeof window === 'undefined') return;

  trackPageview();

  const originalPushState = history.pushState;
  const originalReplaceState = history.replaceState;

  history.pushState = function (...args) {
    originalPushState.apply(this, args);
    trackPageview();
  };

  history.replaceState = function (...args) {
    originalReplaceState.apply(this, args);
    trackPageview();
  };

  window.addEventListener('popstate', () => trackPageview());

  window.addEventListener('beforeunload', () => {
    if (eventQueue.length > 0 && navigator.sendBeacon) {
      navigator.sendBeacon(API_URL, JSON.stringify({ events: eventQueue }));
    }
  });

  if ('PerformanceObserver' in window) {
    try {
      const observer = new PerformanceObserver((list) => {
        const entries = list.getEntries();
        const metrics: Record<string, number> = {};
        
        for (const entry of entries) {
          if (entry.entryType === 'largest-contentful-paint') {
            metrics.lcp = entry.startTime;
          } else if (entry.entryType === 'first-input') {
            metrics.fid = (entry as PerformanceEventTiming).processingStart - entry.startTime;
          } else if (entry.entryType === 'layout-shift' && !(entry as any).hadRecentInput) {
            metrics.cls = (metrics.cls || 0) + (entry as any).value;
          }
        }
        
        if (Object.keys(metrics).length > 0) {
          trackPerformance(metrics);
        }
      });
      
      observer.observe({ type: 'largest-contentful-paint', buffered: true });
      observer.observe({ type: 'first-input', buffered: true });
      observer.observe({ type: 'layout-shift', buffered: true });
    } catch {
      // PerformanceObserver not supported
    }
  }
}

export default {
  init: initAnalytics,
  trackPageview,
  trackClick,
  trackFormSubmit,
  trackError,
  trackPerformance,
};
