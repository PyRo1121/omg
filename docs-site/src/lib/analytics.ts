/**
 * ═══════════════════════════════════════════════════════════════════════════
 * OMG DOCS ANALYTICS - World-Class Web Telemetry
 * Tracks page views, user behavior, and navigation patterns
 * Integrated with admin dashboard at https://api.pyro1121.com
 * ═══════════════════════════════════════════════════════════════════════════
 */

const ANALYTICS_ENDPOINT = 'https://api.pyro1121.com/api/analytics';
const BATCH_SIZE = 10;
const FLUSH_INTERVAL = 5000; // 5 seconds

interface AnalyticsEvent {
  event_type: 'pageview' | 'interaction' | 'navigation' | 'performance';
  event_name: string;
  properties: Record<string, any>;
  timestamp: string;
  session_id: string;
  duration_ms?: number;
}

class DocsAnalytics {
  private eventQueue: AnalyticsEvent[] = [];
  private sessionId: string;
  private flushTimer: NodeJS.Timeout | null = null;
  private pageLoadTime: number;

  constructor() {
    this.sessionId = this.getOrCreateSessionId();
    this.pageLoadTime = performance.now();
    this.startFlushTimer();
    this.setupListeners();
  }

  /**
   * Get or create persistent session ID
   */
  private getOrCreateSessionId(): string {
    const storageKey = 'omg_docs_session';
    let sessionId = sessionStorage.getItem(storageKey);

    if (!sessionId) {
      sessionId = `docs_${Date.now()}_${Math.random().toString(36).substring(2, 11)}`;
      sessionStorage.setItem(storageKey, sessionId);
    }

    return sessionId;
  }

  /**
   * Setup event listeners for automatic tracking
   */
  private setupListeners() {
    // Track page views on navigation
    if (typeof window !== 'undefined') {
      window.addEventListener('popstate', () => this.trackPageView());

      // Track page visibility changes
      document.addEventListener('visibilitychange', () => {
        if (document.hidden) {
          this.track('interaction', 'page_hidden', {});
        } else {
          this.track('interaction', 'page_visible', {});
        }
      });

      // Track outbound clicks
      document.addEventListener('click', (e) => {
        const target = e.target as HTMLElement;
        const link = target.closest('a');

        if (link && link.href) {
          const url = new URL(link.href, window.location.href);

          // External link
          if (url.hostname !== window.location.hostname) {
            this.track('interaction', 'external_click', {
              url: url.href,
              text: link.textContent?.trim().substring(0, 100),
            });
          }
        }
      });

      // Track copy events
      document.addEventListener('copy', () => {
        const selection = window.getSelection()?.toString();
        if (selection && selection.length > 0) {
          this.track('interaction', 'copy_text', {
            length: selection.length,
            snippet: selection.substring(0, 100),
          });
        }
      });
    }
  }

  /**
   * Track a custom event
   */
  track(
    event_type: AnalyticsEvent['event_type'],
    event_name: string,
    properties: Record<string, any>,
    duration_ms?: number
  ) {
    const event: AnalyticsEvent = {
      event_type,
      event_name,
      properties: {
        ...properties,
        url: window.location.pathname,
        referrer: document.referrer,
        viewport: `${window.innerWidth}x${window.innerHeight}`,
        user_agent: navigator.userAgent,
      },
      timestamp: new Date().toISOString(),
      session_id: this.sessionId,
      ...(duration_ms && { duration_ms }),
    };

    this.eventQueue.push(event);

    // Flush immediately if queue is full
    if (this.eventQueue.length >= BATCH_SIZE) {
      this.flush();
    }
  }

  /**
   * Track page view with metadata
   */
  trackPageView() {
    const loadTime = Math.round(performance.now() - this.pageLoadTime);

    // Extract UTM parameters
    const urlParams = new URLSearchParams(window.location.search);
    const utm = {
      source: urlParams.get('utm_source'),
      medium: urlParams.get('utm_medium'),
      campaign: urlParams.get('utm_campaign'),
      term: urlParams.get('utm_term'),
      content: urlParams.get('utm_content'),
    };

    this.track('pageview', 'page_view', {
      title: document.title,
      path: window.location.pathname,
      search: window.location.search,
      hash: window.location.hash,
      utm,
      load_time_ms: loadTime,
    }, loadTime);

    // Reset timer for next page
    this.pageLoadTime = performance.now();
  }

  /**
   * Track navigation event
   */
  trackNavigation(from: string, to: string) {
    this.track('navigation', 'page_transition', {
      from,
      to,
    });
  }

  /**
   * Track search query
   */
  trackSearch(query: string, results_count?: number) {
    this.track('interaction', 'search', {
      query: query.substring(0, 100),
      results_count,
    });
  }

  /**
   * Track code block copy
   */
  trackCodeCopy(language: string, snippet: string) {
    this.track('interaction', 'code_copy', {
      language,
      snippet: snippet.substring(0, 200),
    });
  }

  /**
   * Track sidebar interaction
   */
  trackSidebarClick(section: string, item: string) {
    this.track('interaction', 'sidebar_click', {
      section,
      item,
    });
  }

  /**
   * Track scroll depth (on page unload)
   */
  trackScrollDepth() {
    const scrollDepth = Math.round(
      (window.scrollY / (document.documentElement.scrollHeight - window.innerHeight)) * 100
    );

    this.track('interaction', 'scroll_depth', {
      depth_percent: Math.min(100, scrollDepth),
      max_scroll: window.scrollY,
    });
  }

  /**
   * Flush event queue to API
   */
  private async flush() {
    if (this.eventQueue.length === 0) return;

    const eventsToSend = [...this.eventQueue];
    this.eventQueue = [];

    try {
      await fetch(ANALYTICS_ENDPOINT, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ events: eventsToSend }),
        keepalive: true, // Ensure request completes even if page unloads
      });
    } catch (error) {
      // Silently fail - don't break user experience
      // Only log in development mode
      if (process.env.NODE_ENV === 'development') {
        console.debug('Analytics flush failed:', error);
      }
    }
  }

  /**
   * Start automatic flush timer
   */
  private startFlushTimer() {
    this.flushTimer = setInterval(() => {
      this.flush();
    }, FLUSH_INTERVAL);

    // Flush on page unload
    if (typeof window !== 'undefined') {
      window.addEventListener('beforeunload', () => {
        this.trackScrollDepth();
        this.flush();
      });
    }
  }

  /**
   * Manually flush and cleanup
   */
  destroy() {
    if (this.flushTimer) {
      clearInterval(this.flushTimer);
    }
    this.flush();
  }
}

// Singleton instance
let analyticsInstance: DocsAnalytics | null = null;

/**
 * Initialize analytics (call once on app start)
 */
export function initAnalytics() {
  if (typeof window !== 'undefined' && !analyticsInstance) {
    analyticsInstance = new DocsAnalytics();

    // Track initial page view
    analyticsInstance.trackPageView();
  }

  return analyticsInstance;
}

/**
 * Get analytics instance
 */
export function getAnalytics(): DocsAnalytics | null {
  return analyticsInstance;
}

/**
 * Convenience exports for common tracking
 */
export const analytics = {
  pageView: () => analyticsInstance?.trackPageView(),
  navigation: (from: string, to: string) => analyticsInstance?.trackNavigation(from, to),
  search: (query: string, results?: number) => analyticsInstance?.trackSearch(query, results),
  codeCopy: (lang: string, code: string) => analyticsInstance?.trackCodeCopy(lang, code),
  sidebarClick: (section: string, item: string) => analyticsInstance?.trackSidebarClick(section, item),
  track: (type: AnalyticsEvent['event_type'], name: string, props: Record<string, any>) =>
    analyticsInstance?.track(type, name, props),
};
