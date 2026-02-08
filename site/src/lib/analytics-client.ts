/**
 * Privacy-first analytics client for OMG website
 *
 * Design principles:
 * - No cookies, localStorage, or sessionStorage
 * - No fingerprinting (no canvas, font, or device fingerprints)
 * - No PII collection
 * - GDPR compliant by default
 * - Uses Beacon API for reliable data sending
 * - Geo derived from edge/CDN headers on server
 */

const ANALYTICS_ENDPOINT = 'https://api.pyro1121.com/api/site/analytics/events';
const BATCH_INTERVAL_MS = 3000;
const MAX_BATCH_SIZE = 20;

// Event types for analytics
type EventType =
  | 'pageview'
  | 'scroll_depth'
  | 'time_on_page'
  | 'cta_click'
  | 'web_vitals'
  | 'engagement';

interface AnalyticsEvent {
  type: EventType;
  name: string;
  timestamp: number;
  page_path: string;
  page_url: string;
  properties: Record<string, unknown>;
}

interface WebVitalsMetrics {
  lcp?: number; // Largest Contentful Paint
  inp?: number; // Interaction to Next Paint (replaces FID)
  cls?: number; // Cumulative Layout Shift
  ttfb?: number; // Time to First Byte
  fcp?: number; // First Contentful Paint
}

// Module state (no persistent storage)
let eventQueue: AnalyticsEvent[] = [];
let flushTimer: ReturnType<typeof setTimeout> | null = null;
let pageLoadTime = 0;
let maxScrollDepth = 0;
let isInitialized = false;
let vitalsReported = false;
let clsValue = 0;
let clsEntries: PerformanceEntry[] = [];

/**
 * Generate a simple page view ID for session-less correlation
 * This is NOT a session ID - it's ephemeral and per-page
 */
function generatePageViewId(): string {
  return `pv_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
}

let currentPageViewId = '';

/**
 * Get basic page context without any tracking identifiers
 */
function getPageContext(): { page_path: string; page_url: string } {
  if (typeof window === 'undefined') {
    return { page_path: '', page_url: '' };
  }
  return {
    page_path: window.location.pathname,
    page_url: window.location.href.split('?')[0], // Strip query params for privacy
  };
}

/**
 * Get referrer domain only (not full URL for privacy)
 */
function getReferrerDomain(): string {
  if (typeof document === 'undefined' || !document.referrer) {
    return 'direct';
  }
  try {
    const url = new URL(document.referrer);
    if (url.hostname === window.location.hostname) {
      return 'internal';
    }
    return url.hostname;
  } catch {
    return 'direct';
  }
}

/**
 * Get UTM parameters from current URL
 */
function getUtmParams(): Record<string, string> {
  if (typeof window === 'undefined') return {};
  const params = new URLSearchParams(window.location.search);
  const utm: Record<string, string> = {};
  const utmKeys = ['utm_source', 'utm_medium', 'utm_campaign', 'utm_content', 'utm_term'];
  for (const key of utmKeys) {
    const value = params.get(key);
    if (value) {
      utm[key.replace('utm_', '')] = value;
    }
  }
  return utm;
}

/**
 * Queue an analytics event
 */
function queueEvent(type: EventType, name: string, properties: Record<string, unknown> = {}): void {
  const context = getPageContext();
  const event: AnalyticsEvent = {
    type,
    name,
    timestamp: Date.now(),
    page_path: context.page_path,
    page_url: context.page_url,
    properties: {
      ...properties,
      pv_id: currentPageViewId,
    },
  };
  eventQueue.push(event);

  // Auto-flush if batch is full
  if (eventQueue.length >= MAX_BATCH_SIZE) {
    flushEvents();
  } else {
    scheduleFlush();
  }
}

/**
 * Schedule a flush of the event queue
 */
function scheduleFlush(): void {
  if (flushTimer) return;
  flushTimer = setTimeout(() => {
    flushTimer = null;
    flushEvents();
  }, BATCH_INTERVAL_MS);
}

/**
 * Flush events using Beacon API for reliability
 */
function flushEvents(): void {
  if (eventQueue.length === 0) return;

  const events = [...eventQueue];
  eventQueue = [];

  const payload = JSON.stringify({ events });

  // Prefer Beacon API for reliability (works even on page unload)
  if (navigator.sendBeacon) {
    const success = navigator.sendBeacon(ANALYTICS_ENDPOINT, payload);
    if (!success) {
      // Beacon failed, try fetch as fallback
      sendWithFetch(payload, events);
    }
  } else {
    sendWithFetch(payload, events);
  }
}

/**
 * Fallback to fetch if Beacon API unavailable
 */
async function sendWithFetch(payload: string, events: AnalyticsEvent[]): Promise<void> {
  try {
    const response = await fetch(ANALYTICS_ENDPOINT, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: payload,
      keepalive: true,
    });
    if (!response.ok) {
      // Re-queue events on failure
      eventQueue = [...events, ...eventQueue];
    }
  } catch {
    // Re-queue events on error
    eventQueue = [...events, ...eventQueue];
  }
}

// ============================================================================
// PUBLIC API
// ============================================================================

/**
 * Track a page view
 */
export function trackPageView(): void {
  if (typeof window === 'undefined') return;

  currentPageViewId = generatePageViewId();
  pageLoadTime = Date.now();
  maxScrollDepth = 0;
  vitalsReported = false;
  clsValue = 0;
  clsEntries = [];

  const viewport = {
    width: window.innerWidth,
    height: window.innerHeight,
  };

  queueEvent('pageview', 'page_view', {
    referrer: getReferrerDomain(),
    utm: getUtmParams(),
    viewport,
    device_type: viewport.width < 768 ? 'mobile' : viewport.width < 1024 ? 'tablet' : 'desktop',
  });
}

/**
 * Track scroll depth (call on scroll events, deduplicated internally)
 */
export function trackScrollDepth(depth: number): void {
  // Only track at specific thresholds: 25%, 50%, 75%, 90%, 100%
  const thresholds = [25, 50, 75, 90, 100];
  const roundedDepth = thresholds.find((t) => depth >= t && maxScrollDepth < t);

  if (roundedDepth && roundedDepth > maxScrollDepth) {
    maxScrollDepth = roundedDepth;
    queueEvent('scroll_depth', 'scroll', {
      depth: roundedDepth,
    });
  }
}

/**
 * Track time spent on page (call on page unload)
 */
export function trackTimeOnPage(): void {
  if (pageLoadTime === 0) return;

  const timeSpentMs = Date.now() - pageLoadTime;
  const timeSpentSec = Math.round(timeSpentMs / 1000);

  // Only track if user spent meaningful time (> 5 seconds)
  if (timeSpentSec >= 5) {
    queueEvent('time_on_page', 'time_spent', {
      duration_seconds: timeSpentSec,
      max_scroll_depth: maxScrollDepth,
    });
  }
}

/**
 * Track CTA clicks
 */
export function trackCtaClick(
  ctaType: 'download' | 'signup' | 'pricing' | 'docs' | 'github' | 'install',
  ctaLabel?: string
): void {
  queueEvent('cta_click', 'cta_interaction', {
    cta_type: ctaType,
    cta_label: ctaLabel || ctaType,
  });
}

/**
 * Track custom engagement events
 */
export function trackEngagement(action: string, properties?: Record<string, unknown>): void {
  queueEvent('engagement', action, properties || {});
}

/**
 * Report Core Web Vitals
 */
export function reportWebVitals(metrics: WebVitalsMetrics): void {
  if (vitalsReported) return;
  vitalsReported = true;

  const vitalsWithRating: Record<string, unknown> = {};

  // Add values and ratings for each metric
  if (metrics.lcp !== undefined) {
    vitalsWithRating.lcp = metrics.lcp;
    vitalsWithRating.lcp_rating = metrics.lcp <= 2500 ? 'good' : metrics.lcp <= 4000 ? 'needs-improvement' : 'poor';
  }
  if (metrics.inp !== undefined) {
    vitalsWithRating.inp = metrics.inp;
    vitalsWithRating.inp_rating = metrics.inp <= 200 ? 'good' : metrics.inp <= 500 ? 'needs-improvement' : 'poor';
  }
  if (metrics.cls !== undefined) {
    vitalsWithRating.cls = metrics.cls;
    vitalsWithRating.cls_rating = metrics.cls <= 0.1 ? 'good' : metrics.cls <= 0.25 ? 'needs-improvement' : 'poor';
  }
  if (metrics.ttfb !== undefined) {
    vitalsWithRating.ttfb = metrics.ttfb;
    vitalsWithRating.ttfb_rating = metrics.ttfb <= 800 ? 'good' : metrics.ttfb <= 1800 ? 'needs-improvement' : 'poor';
  }
  if (metrics.fcp !== undefined) {
    vitalsWithRating.fcp = metrics.fcp;
    vitalsWithRating.fcp_rating = metrics.fcp <= 1800 ? 'good' : metrics.fcp <= 3000 ? 'needs-improvement' : 'poor';
  }

  queueEvent('web_vitals', 'core_web_vitals', vitalsWithRating);
}

/**
 * Initialize scroll depth tracking
 */
function initScrollTracking(): void {
  let ticking = false;

  const handleScroll = () => {
    if (ticking) return;
    ticking = true;

    requestAnimationFrame(() => {
      const scrollHeight = document.documentElement.scrollHeight - window.innerHeight;
      if (scrollHeight > 0) {
        const scrollPercentage = Math.round((window.scrollY / scrollHeight) * 100);
        trackScrollDepth(scrollPercentage);
      }
      ticking = false;
    });
  };

  window.addEventListener('scroll', handleScroll, { passive: true });
}

/**
 * Initialize Web Vitals collection using PerformanceObserver
 */
function initWebVitalsCollection(): void {
  if (!('PerformanceObserver' in window)) return;

  const metrics: WebVitalsMetrics = {};
  let lcpReported = false;
  let inpReported = false;

  // LCP Observer
  try {
    const lcpObserver = new PerformanceObserver((list) => {
      const entries = list.getEntries();
      const lastEntry = entries[entries.length - 1];
      if (lastEntry) {
        metrics.lcp = lastEntry.startTime;
      }
    });
    lcpObserver.observe({ type: 'largest-contentful-paint', buffered: true });

    // Report LCP on first interaction or visibility change
    const reportLcp = () => {
      if (!lcpReported && metrics.lcp) {
        lcpReported = true;
        lcpObserver.disconnect();
      }
    };
    ['keydown', 'click', 'visibilitychange'].forEach((event) => {
      window.addEventListener(event, reportLcp, { once: true, capture: true });
    });
  } catch {
    // LCP observer not supported
  }

  // INP Observer (replaces FID)
  try {
    const inpObserver = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        const eventEntry = entry as PerformanceEventTiming;
        const duration = eventEntry.processingEnd - eventEntry.startTime;
        if (!metrics.inp || duration > metrics.inp) {
          metrics.inp = duration;
        }
      }
    });
    inpObserver.observe({ type: 'event', buffered: true });
  } catch {
    // INP observer not supported, try FID as fallback
    try {
      const fidObserver = new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          const eventEntry = entry as PerformanceEventTiming;
          metrics.inp = eventEntry.processingStart - eventEntry.startTime;
        }
      });
      fidObserver.observe({ type: 'first-input', buffered: true });
    } catch {
      // Neither supported
    }
  }

  // CLS Observer
  try {
    const clsObserver = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        const layoutShift = entry as PerformanceEntry & { hadRecentInput?: boolean; value?: number };
        if (!layoutShift.hadRecentInput && layoutShift.value) {
          clsValue += layoutShift.value;
          clsEntries.push(entry);
        }
      }
      metrics.cls = clsValue;
    });
    clsObserver.observe({ type: 'layout-shift', buffered: true });
  } catch {
    // CLS observer not supported
  }

  // TTFB and FCP from navigation timing
  try {
    const paintObserver = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        if (entry.name === 'first-contentful-paint') {
          metrics.fcp = entry.startTime;
        }
      }
    });
    paintObserver.observe({ type: 'paint', buffered: true });

    // Get TTFB from navigation timing
    const navEntry = performance.getEntriesByType('navigation')[0] as PerformanceNavigationTiming;
    if (navEntry) {
      metrics.ttfb = navEntry.responseStart - navEntry.requestStart;
    }
  } catch {
    // Navigation timing not available
  }

  // Report vitals before page unload
  const reportVitals = () => {
    if (Object.keys(metrics).length > 0) {
      reportWebVitals(metrics);
    }
  };

  window.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') {
      reportVitals();
    }
  });

  window.addEventListener('pagehide', reportVitals);
}

/**
 * Initialize CTA click tracking
 */
function initCtaTracking(): void {
  document.addEventListener(
    'click',
    (e) => {
      const target = e.target as HTMLElement;
      const link = target.closest('a, button');
      if (!link) return;

      const href = link.getAttribute('href') || '';
      const dataTrack = link.getAttribute('data-track-cta');

      // Auto-detect CTA type from href or data attribute
      if (dataTrack) {
        trackCtaClick(dataTrack as Parameters<typeof trackCtaClick>[0], link.textContent?.trim());
      } else if (href.includes('install') || href.includes('#install')) {
        trackCtaClick('install', link.textContent?.trim());
      } else if (href.includes('signup') || href.includes('login')) {
        trackCtaClick('signup', link.textContent?.trim());
      } else if (href.includes('pricing') || href.includes('#pricing')) {
        trackCtaClick('pricing', link.textContent?.trim());
      } else if (href.includes('/docs')) {
        trackCtaClick('docs', link.textContent?.trim());
      } else if (href.includes('github.com')) {
        trackCtaClick('github', link.textContent?.trim());
      } else if (href.includes('download') || link.classList.contains('download-btn')) {
        trackCtaClick('download', link.textContent?.trim());
      }
    },
    { capture: true }
  );
}

/**
 * Initialize SPA navigation tracking
 */
function initNavigationTracking(): void {
  // Track history changes for SPA navigation
  const originalPushState = history.pushState;
  const originalReplaceState = history.replaceState;

  history.pushState = function (...args) {
    // Track time on previous page before navigation
    trackTimeOnPage();
    flushEvents();

    originalPushState.apply(this, args);
    trackPageView();
  };

  history.replaceState = function (...args) {
    originalReplaceState.apply(this, args);
    // Don't track pageview on replaceState (e.g., query param updates)
  };

  window.addEventListener('popstate', () => {
    trackTimeOnPage();
    flushEvents();
    trackPageView();
  });
}

/**
 * Initialize all analytics tracking
 */
export function initAnalytics(): void {
  if (typeof window === 'undefined' || isInitialized) return;
  isInitialized = true;

  // Track initial page view
  trackPageView();

  // Initialize all trackers
  initScrollTracking();
  initWebVitalsCollection();
  initCtaTracking();
  initNavigationTracking();

  // Flush on page unload
  window.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') {
      trackTimeOnPage();
      flushEvents();
    }
  });

  window.addEventListener('pagehide', () => {
    trackTimeOnPage();
    flushEvents();
  });

  // Fallback for beforeunload
  window.addEventListener('beforeunload', () => {
    trackTimeOnPage();
    flushEvents();
  });
}

export default {
  init: initAnalytics,
  trackPageView,
  trackScrollDepth,
  trackTimeOnPage,
  trackCtaClick,
  trackEngagement,
  reportWebVitals,
};
