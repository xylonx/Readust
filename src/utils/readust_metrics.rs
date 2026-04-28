use metrics::{Unit, describe_counter, describe_gauge, describe_histogram};
use metrics_exporter_prometheus::{BuildError, Matcher, PrometheusBuilder, PrometheusRecorder};

pub struct Metric {
    pub name: &'static str,
    description: &'static str,
}

pub const METRICS_COUNTER: &[Metric] = &[HTTP_REQUESTS_TOTAL];
pub const METRICS_GAUGE: &[Metric] = &[HTTP_REQUESTS_IN_FLIGHT];
pub const METRICS_HISTOGRAM: &[Metric] = &[HTTP_REQUEST_DURATION_SECONDS];

pub const HTTP_REQUESTS_IN_FLIGHT: Metric = Metric {
    name: "http_requests_in_flight",
    description: "Number of HTTP requests currently being processed.",
};

pub const HTTP_REQUESTS_TOTAL: Metric = Metric {
    name: "http_requests_total",
    description: "Total number of HTTP requests processed.",
};

pub const HTTP_REQUEST_DURATION_SECONDS: Metric = Metric {
    name: "http_request_duration_seconds",
    description: "Duration of HTTP request processing in seconds.",
};

pub fn setup_prometheus_metrics_recorder() -> Result<PrometheusRecorder, BuildError> {
    const EXPONENTIAL_SECONDS: &[f64] = &[
        0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
    ];

    for couter in METRICS_COUNTER {
        describe_counter!(couter.name, Unit::Count, couter.description);
    }

    for gauge in METRICS_GAUGE {
        describe_gauge!(gauge.name, Unit::Count, gauge.description);
    }

    for histogram in METRICS_HISTOGRAM {
        describe_histogram!(histogram.name, Unit::Seconds, histogram.description);
    }

    let mut builder = PrometheusBuilder::new();
    for histogram in METRICS_HISTOGRAM {
        builder = builder.set_buckets_for_metric(
            Matcher::Full(histogram.name.to_string()),
            EXPONENTIAL_SECONDS,
        )?;
    }

    Ok(builder.build_recorder())
}
