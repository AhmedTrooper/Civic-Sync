use crate::error::ApiError;
use axum::{extract::Request, middleware::Next, response::Response};

pub async fn rbac_middleware(req: Request, next: Next) -> Result<Response, ApiError> {
    let method = req.method().clone();

    // Enforce RBAC on mutating requests (POST, PUT, PATCH, DELETE)
    if method == axum::http::Method::POST
        || method == axum::http::Method::PUT
        || method == axum::http::Method::PATCH
        || method == axum::http::Method::DELETE
    {
        let role_str = req
            .headers()
            .get("x-role")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("viewer");

        let allowed = match role_str.to_lowercase().as_str() {
            "admin" => true,
            "dispatcher" => true,
            _ => false, // viewers and unauthenticated users are blocked from mutations
        };

        if !allowed {
            return Err(ApiError::Forbidden(format!(
                "Insufficient permissions. Role '{}' cannot perform mutations.",
                role_str
            )));
        }
    }

    Ok(next.run(req).await)
}
