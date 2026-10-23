use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use devtools_ab_cutover::{assign, AssignmentRequest, InfraiClient, ServiceError};
use serde_json::json;
use std::sync::Arc;

async fn assignment(State(client): State<Arc<InfraiClient>>, Json(input): Json<AssignmentRequest>) -> impl IntoResponse {
    match assign(&client, input).await {
        Ok(result) => (StatusCode::OK, Json(json!({"ok": true, "data": result}))).into_response(),
        Err(error @ ServiceError::Api { status, .. }) => (StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY), Json(json!({"ok": false, "error": error.to_string()}))).into_response(),
        Err(error) => (StatusCode::BAD_GATEWAY, Json(json!({"ok": false, "error": error.to_string()}))).into_response(),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Arc::new(InfraiClient::from_env()?);
    let app = Router::new().route("/assign", post(assignment)).with_state(client);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("listening on http://127.0.0.1:3000");
    axum::serve(listener, app).await?;
    Ok(())
}

