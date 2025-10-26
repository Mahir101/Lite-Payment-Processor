use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use crate::metrics;

/// HTTP metrics middleware that records request metrics
pub async fn metrics_middleware(request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    
    // Extract endpoint path (remove query parameters)
    let endpoint = uri.path().to_string();
    
    // Process the request
    let response = next.run(request).await;
    
    // Record metrics
    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16();
    
    // Record HTTP request metrics
    metrics::increment_http_request(
        method.as_str(),
        &endpoint,
        status,
    );
    
    metrics::record_http_duration(
        method.as_str(),
        &endpoint,
        duration,
    );
    
    response
}
