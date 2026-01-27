//! System-wide metrics collection (Prometheus-style)
//!
//! Provides atomic counters and gauges for monitoring system health and security.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

/// Snapshot of current metrics state
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub requests_total: u64,
    pub requests_failed: u64,
    pub rate_limit_hits: u64,
    pub validation_failures: u64,
    pub active_connections: i64,
    pub security_audit_requests: u64,
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub search_requests: u64,
    pub info_requests: u64,
    pub status_requests: u64,
}

/// Global metrics registry using atomics for high performance
pub struct Metrics {
    requests_total: AtomicU64,
    requests_failed: AtomicU64,
    rate_limit_hits: AtomicU64,
    validation_failures: AtomicU64,
    active_connections: AtomicI64,
    security_audit_requests: AtomicU64,
    bytes_received: AtomicU64,
    bytes_sent: AtomicU64,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    search_requests: AtomicU64,
    info_requests: AtomicU64,
    status_requests: AtomicU64,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    pub const fn new() -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            requests_failed: AtomicU64::new(0),
            rate_limit_hits: AtomicU64::new(0),
            validation_failures: AtomicU64::new(0),
            active_connections: AtomicI64::new(0),
            security_audit_requests: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            search_requests: AtomicU64::new(0),
            info_requests: AtomicU64::new(0),
            status_requests: AtomicU64::new(0),
        }
    }

    pub fn inc_requests_total(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_requests_failed(&self) {
        self.requests_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_rate_limit_hits(&self) {
        self.rate_limit_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_validation_failures(&self) {
        self.validation_failures.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_active_connections(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec_active_connections(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn inc_security_audit_requests(&self) {
        self.security_audit_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_bytes_received(&self, bytes: u64) {
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn add_bytes_sent(&self, bytes: u64) {
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn inc_cache_hits(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_cache_misses(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_search_requests(&self) {
        self.search_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_info_requests(&self) {
        self.info_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_status_requests(&self) {
        self.status_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            requests_total: self.requests_total.load(Ordering::Relaxed),
            requests_failed: self.requests_failed.load(Ordering::Relaxed),
            rate_limit_hits: self.rate_limit_hits.load(Ordering::Relaxed),
            validation_failures: self.validation_failures.load(Ordering::Relaxed),
            active_connections: self.active_connections.load(Ordering::Relaxed),
            security_audit_requests: self.security_audit_requests.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            search_requests: self.search_requests.load(Ordering::Relaxed),
            info_requests: self.info_requests.load(Ordering::Relaxed),
            status_requests: self.status_requests.load(Ordering::Relaxed),
        }
    }
}

/// Global singleton for metrics
pub static GLOBAL_METRICS: Metrics = Metrics::new();
