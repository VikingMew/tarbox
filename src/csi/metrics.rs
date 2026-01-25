use prometheus::{CounterVec, Gauge, GaugeVec, HistogramOpts, HistogramVec, Opts, Registry};
use std::sync::Arc;

/// CSI metrics collector
pub struct CsiMetrics {
    /// Total CSI operations
    pub operations_total: CounterVec,
    /// CSI operation duration in seconds
    pub operation_duration: HistogramVec,
    /// CSI operation errors
    pub operation_errors: CounterVec,
    /// Number of volumes
    pub volume_count: GaugeVec,
    /// Volume capacity in bytes
    pub volume_capacity: GaugeVec,
    /// Volume used bytes
    pub volume_used: GaugeVec,
    /// Number of active mounts
    pub mount_count: GaugeVec,
    /// Mount duration in seconds
    pub mount_duration: HistogramVec,
    /// Number of snapshots
    pub snapshot_count: Gauge,
    /// Snapshot size in bytes
    pub snapshot_size: GaugeVec,
}

impl CsiMetrics {
    pub fn new(registry: Arc<Registry>) -> Result<Self, prometheus::Error> {
        let operations_total = CounterVec::new(
            Opts::new("tarbox_csi_operations_total", "Total CSI operations"),
            &["method"],
        )?;

        let operation_duration = HistogramVec::new(
            HistogramOpts::new(
                "tarbox_csi_operation_duration_seconds",
                "CSI operation duration in seconds",
            ),
            &["method"],
        )?;

        let operation_errors = CounterVec::new(
            Opts::new("tarbox_csi_operation_errors_total", "CSI operation errors"),
            &["method"],
        )?;

        let volume_count =
            GaugeVec::new(Opts::new("tarbox_volume_count", "Number of volumes"), &["namespace"])?;

        let volume_capacity = GaugeVec::new(
            Opts::new("tarbox_volume_capacity_bytes", "Volume capacity in bytes"),
            &["volume_id"],
        )?;

        let volume_used = GaugeVec::new(
            Opts::new("tarbox_volume_used_bytes", "Volume used bytes"),
            &["volume_id"],
        )?;

        let mount_count =
            GaugeVec::new(Opts::new("tarbox_mount_count", "Number of active mounts"), &["node"])?;

        let mount_duration = HistogramVec::new(
            HistogramOpts::new("tarbox_mount_duration_seconds", "Mount duration in seconds"),
            &["node"],
        )?;

        let snapshot_count = Gauge::new("tarbox_snapshot_count", "Number of snapshots")?;

        let snapshot_size = GaugeVec::new(
            Opts::new("tarbox_snapshot_size_bytes", "Snapshot size in bytes"),
            &["snapshot_id"],
        )?;

        registry.register(Box::new(operations_total.clone()))?;
        registry.register(Box::new(operation_duration.clone()))?;
        registry.register(Box::new(operation_errors.clone()))?;
        registry.register(Box::new(volume_count.clone()))?;
        registry.register(Box::new(volume_capacity.clone()))?;
        registry.register(Box::new(volume_used.clone()))?;
        registry.register(Box::new(mount_count.clone()))?;
        registry.register(Box::new(mount_duration.clone()))?;
        registry.register(Box::new(snapshot_count.clone()))?;
        registry.register(Box::new(snapshot_size.clone()))?;

        Ok(Self {
            operations_total,
            operation_duration,
            operation_errors,
            volume_count,
            volume_capacity,
            volume_used,
            mount_count,
            mount_duration,
            snapshot_count,
            snapshot_size,
        })
    }

    /// Record an operation
    pub fn record_operation(&self, method: &str, duration_secs: f64, success: bool) {
        self.operations_total.with_label_values(&[method]).inc();
        self.operation_duration.with_label_values(&[method]).observe(duration_secs);
        if !success {
            self.operation_errors.with_label_values(&[method]).inc();
        }
    }

    /// Update volume metrics
    pub fn update_volume(&self, volume_id: &str, namespace: &str, capacity: i64, used: i64) {
        self.volume_capacity.with_label_values(&[volume_id]).set(capacity as f64);
        self.volume_used.with_label_values(&[volume_id]).set(used as f64);
        // Update namespace count (approximate)
        self.volume_count.with_label_values(&[namespace]).inc();
    }

    /// Remove volume metrics
    pub fn remove_volume(&self, volume_id: &str, namespace: &str) {
        let _ = self.volume_capacity.remove_label_values(&[volume_id]);
        let _ = self.volume_used.remove_label_values(&[volume_id]);
        self.volume_count.with_label_values(&[namespace]).dec();
    }

    /// Update mount metrics
    pub fn update_mount(&self, node: &str, count: i64, duration_secs: f64) {
        self.mount_count.with_label_values(&[node]).set(count as f64);
        self.mount_duration.with_label_values(&[node]).observe(duration_secs);
    }

    /// Update snapshot metrics
    pub fn update_snapshot(&self, snapshot_id: &str, size: i64) {
        self.snapshot_count.inc();
        self.snapshot_size.with_label_values(&[snapshot_id]).set(size as f64);
    }

    /// Remove snapshot metrics
    pub fn remove_snapshot(&self, snapshot_id: &str) {
        self.snapshot_count.dec();
        let _ = self.snapshot_size.remove_label_values(&[snapshot_id]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csi_metrics_creation() {
        let registry = Arc::new(Registry::new());
        let metrics = CsiMetrics::new(registry).unwrap();

        metrics.record_operation("CreateVolume", 1.5, true);
        assert_eq!(metrics.operations_total.with_label_values(&["CreateVolume"]).get(), 1.0);
    }

    #[test]
    fn test_volume_metrics() {
        let registry = Arc::new(Registry::new());
        let metrics = CsiMetrics::new(registry).unwrap();

        metrics.update_volume("vol-1", "default", 1000, 500);
        assert_eq!(metrics.volume_capacity.with_label_values(&["vol-1"]).get(), 1000.0);
        assert_eq!(metrics.volume_used.with_label_values(&["vol-1"]).get(), 500.0);
    }

    #[test]
    fn test_snapshot_metrics() {
        let registry = Arc::new(Registry::new());
        let metrics = CsiMetrics::new(registry).unwrap();

        metrics.update_snapshot("snap-1", 2000);
        assert_eq!(metrics.snapshot_count.get(), 1.0);

        metrics.remove_snapshot("snap-1");
        assert_eq!(metrics.snapshot_count.get(), 0.0);
    }
}
