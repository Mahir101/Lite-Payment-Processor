//! # Rate Limiting Middleware
//! 
//! This module provides rate limiting functionality using PostgreSQL
//! to track API request counts per endpoint and API key.

use actix_web::{
    dev::{ServiceRequest, ServiceResponse, Transform},
    Error,
};
use actix_web::dev::{Service, ServiceFactory};
use futures_util::future::{ok, Ready};
use sqlx::PgPool;
use std::{
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::Arc,
    task::{Context, Poll},
};

pub struct RateLimiter {
    pool: PgPool,
    default_limit: u32,
    window_seconds: u64,
}

impl RateLimiter {
    pub fn new(pool: PgPool, default_limit: u32, window_seconds: u64) -> Self {
        Self {
            pool,
            default_limit,
            window_seconds,
        }
    }

    pub async fn check_rate_limit(
        &self,
        api_key: &str,
        endpoint: &str,
    ) -> Result<RateLimitResult, sqlx::Error> {
        let window_start = chrono::Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        let limit = self.get_limit_for_endpoint(endpoint).await.unwrap_or(self.default_limit);

        let row = sqlx::query(
            r#"
            SELECT request_count, limit_per_window
            FROM api_rate_limits
            WHERE api_key = $1 AND endpoint = $2 AND window_start = $3
            "#
        )
        .bind(api_key)
        .bind(endpoint)
        .bind(window_start)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let current_count: i32 = row.get("request_count");
                let limit_per_window: i32 = row.get("limit_per_window");

                if current_count >= limit_per_window {
                    Ok(RateLimitResult {
                        allowed: false,
                        remaining: 0,
                        reset_at: window_start + chrono::Duration::seconds(self.window_seconds as i64),
                    })
                } else {
                    sqlx::query(
                        r#"
                        UPDATE api_rate_limits
                        SET request_count = request_count + 1
                        WHERE api_key = $1 AND endpoint = $2 AND window_start = $3
                        "#
                    )
                    .bind(api_key)
                    .bind(endpoint)
                    .bind(window_start)
                    .execute(&self.pool)
                    .await?;

                    Ok(RateLimitResult {
                        allowed: true,
                        remaining: (limit_per_window - current_count - 1) as u32,
                        reset_at: window_start + chrono::Duration::seconds(self.window_seconds as i64),
                    })
                }
            }
            None => {
                sqlx::query(
                    r#"
                    INSERT INTO api_rate_limits (api_key, endpoint, request_count, limit_per_window, window_start)
                    VALUES ($1, $2, 1, $3, $4)
                    ON CONFLICT (api_key, endpoint, window_start) DO UPDATE
                    SET request_count = api_rate_limits.request_count + 1
                    "#
                )
                .bind(api_key)
                .bind(endpoint)
                .bind(limit as i32)
                .bind(window_start)
                .execute(&self.pool)
                .await?;

                Ok(RateLimitResult {
                    allowed: true,
                    remaining: limit - 1,
                    reset_at: window_start + chrono::Duration::seconds(self.window_seconds as i64),
                })
            }
        }
    }

    async fn get_limit_for_endpoint(&self, endpoint: &str) -> Option<u32> {
        None
    }
}

#[derive(Debug, Clone)]
pub struct RateLimitResult {
    pub allowed: bool,
    pub remaining: u32,
    pub reset_at: chrono::DateTime<chrono::Utc>,
}

pub struct RateLimitMiddleware {
    rate_limiter: Arc<RateLimiter>,
}

impl RateLimitMiddleware {
    pub fn new(rate_limiter: Arc<RateLimiter>) -> Self {
        Self { rate_limiter }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimitMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RateLimitMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RateLimitMiddlewareService {
            service: Rc::new(service),
            rate_limiter: self.rate_limiter.clone(),
        })
    }
}

pub struct RateLimitMiddlewareService<S> {
    service: Rc<S>,
    rate_limiter: Arc<RateLimiter>,
}

impl<S, B> Service<ServiceRequest> for RateLimitMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let api_key = req.headers()
            .get("X-API-Key")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("default")
            .to_string();
        let endpoint = req.path().to_string();
        let service = self.service.clone();
        let rate_limiter = self.rate_limiter.clone();

        Box::pin(async move {
            let result = rate_limiter.check_rate_limit(&api_key, &endpoint).await;
            
            match result {
                Ok(limit_result) => {
                    if !limit_result.allowed {
                        let mut res = ServiceResponse::new(
                            req.into_parts().0,
                            actix_web::HttpResponse::TooManyRequests().json(serde_json::json!({
                                "error": "Rate limit exceeded",
                                "remaining": limit_result.remaining,
                                "reset_at": limit_result.reset_at
                            }))
                        );
                        return Ok(res);
                    }

                    let res = service.call(req).await?;
                    let mut res = res.map_into_boxed_body();
                    
                    // Add rate limit headers
                    res.headers_mut().insert(
                        "X-RateLimit-Remaining",
                        limit_result.remaining.to_string().parse().unwrap(),
                    );
                    res.headers_mut().insert(
                        "X-RateLimit-Reset",
                        limit_result.reset_at.timestamp().to_string().parse().unwrap(),
                    );

                    Ok(ServiceResponse::new(res.into_parts().0, res.into_parts().1))
                }
                Err(_) => {
                    // On error, allow request but log it
                    service.call(req).await
                }
            }
        })
    }
}
