//! Prometheus metrics collection and exposition.

use std::sync::atomic::{AtomicU64, Ordering};

/// Process-wide metrics tracked using atomic counters.
#[derive(Debug, Default)]
pub struct AppMetrics {
    pub http_requests_total: AtomicU64,
    pub redirects_total: AtomicU64,
    pub links_created_total: AtomicU64,
    pub links_deleted_total: AtomicU64,
    pub cache_hits_total: AtomicU64,
    pub cache_misses_total: AtomicU64,
}

impl AppMetrics {
    /// Render current counter values in Prometheus text exposition format.
    pub fn render_prometheus(&self) -> String {
        format!(
            "# HELP rlnk_http_requests_total Total number of HTTP requests processed.\n\
             # TYPE rlnk_http_requests_total counter\n\
             rlnk_http_requests_total {}\n\
             # HELP rlnk_redirects_total Total number of URL redirects served.\n\
             # TYPE rlnk_redirects_total counter\n\
             rlnk_redirects_total {}\n\
             # HELP rlnk_links_created_total Total number of short links created.\n\
             # TYPE rlnk_links_created_total counter\n\
             rlnk_links_created_total {}\n\
             # HELP rlnk_links_deleted_total Total number of short links deleted.\n\
             # TYPE rlnk_links_deleted_total counter\n\
             rlnk_links_deleted_total {}\n\
             # HELP rlnk_cache_hits_total Total number of recent access cache hits.\n\
             # TYPE rlnk_cache_hits_total counter\n\
             rlnk_cache_hits_total {}\n\
             # HELP rlnk_cache_misses_total Total number of recent access cache misses.\n\
             # TYPE rlnk_cache_misses_total counter\n\
             rlnk_cache_misses_total {}\n",
            self.http_requests_total.load(Ordering::Relaxed),
            self.redirects_total.load(Ordering::Relaxed),
            self.links_created_total.load(Ordering::Relaxed),
            self.links_deleted_total.load(Ordering::Relaxed),
            self.cache_hits_total.load(Ordering::Relaxed),
            self.cache_misses_total.load(Ordering::Relaxed),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::AppMetrics;

    #[test]
    fn render_prometheus_should_output_valid_prometheus_format() {
        let metrics = AppMetrics::default();
        metrics.http_requests_total.store(10, Ordering::Relaxed);
        metrics.redirects_total.store(7, Ordering::Relaxed);
        metrics.links_created_total.store(3, Ordering::Relaxed);
        metrics.links_deleted_total.store(1, Ordering::Relaxed);
        metrics.cache_hits_total.store(5, Ordering::Relaxed);
        metrics.cache_misses_total.store(2, Ordering::Relaxed);

        let rendered = metrics.render_prometheus();
        assert!(rendered.contains("rlnk_http_requests_total 10\n"));
        assert!(rendered.contains("rlnk_redirects_total 7\n"));
        assert!(rendered.contains("rlnk_links_created_total 3\n"));
        assert!(rendered.contains("rlnk_links_deleted_total 1\n"));
        assert!(rendered.contains("rlnk_cache_hits_total 5\n"));
        assert!(rendered.contains("rlnk_cache_misses_total 2\n"));
    }
}
