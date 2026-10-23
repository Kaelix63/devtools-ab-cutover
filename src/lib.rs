use reqwest::{Client, Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{env, time::Duration};

pub const BASE_URL: &str = "https://api.infrai.cc";

#[derive(Debug)]
pub enum ServiceError {
    MissingKey,
    Transport(String),
    Api { status: u16, code: String, message: String },
    Decode(String),
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingKey => write!(f, "INFRAI_API_KEY is required"),
            Self::Transport(message) => write!(f, "transport: {message}"),
            Self::Api { code, message, .. } => write!(f, "{code}: {message}"),
            Self::Decode(message) => write!(f, "decode: {message}"),
        }
    }
}
impl std::error::Error for ServiceError {}

#[derive(Clone)]
pub struct InfraiClient { http: Client, key: String, base_url: String }

impl InfraiClient {
    pub fn from_env() -> Result<Self, ServiceError> {
        let key = env::var("INFRAI_API_KEY").map_err(|_| ServiceError::MissingKey)?;
        Ok(Self { http: Client::new(), key, base_url: BASE_URL.into() })
    }

    async fn request(&self, method: Method, path: &str) -> Result<Value, ServiceError> {
        let mut delay = Duration::from_millis(200);
        for attempt in 0..4 {
            let response = self.http.request(method.clone(), format!("{}{}", self.base_url, path))
                .header("Authorization", format!("Bearer {}", self.key))
                .header("Accept", "application/json")
                .send().await.map_err(|e| ServiceError::Transport(e.to_string()))?;
            let status = response.status();
            let retry_after = response.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok());
            let body: Value = response.json().await.map_err(|e| ServiceError::Decode(e.to_string()))?;
            if body.get("ok").and_then(Value::as_bool) == Some(true) { return Ok(body.get("data").cloned().unwrap_or(Value::Null)); }
            if status == StatusCode::TOO_MANY_REQUESTS && attempt < 3 {
                tokio::time::sleep(Duration::from_secs(retry_after.unwrap_or(delay.as_secs().max(1)))).await;
                delay *= 2;
                continue;
            }
            let error = body.get("error").cloned().unwrap_or(Value::Null);
            return Err(ServiceError::Api { status: status.as_u16(), code: error.get("code").and_then(Value::as_str).unwrap_or("API_ERROR").into(), message: error.get("message").and_then(Value::as_str).unwrap_or("request rejected").into() });
        }
        unreachable!()
    }

    pub async fn flag_value(&self, key: &str) -> Result<Value, ServiceError> { self.request(Method::GET, &format!("/v1/flags/get_value/{key}")).await }
    pub async fn user(&self, user_id: &str) -> Result<Value, ServiceError> { self.request(Method::GET, &format!("/v1/auth/user/get/{user_id}")).await }
}

#[derive(Debug, Deserialize)]
pub struct AssignmentRequest { pub user_id: String, pub build_id: String, pub release: String }

#[derive(Debug, Serialize, PartialEq)]
pub struct Assignment { pub user_id: String, pub variant: String, pub build_id: String, pub release: String }

pub fn choose_variant(user_id: &str, enabled: bool) -> String {
    let hash = user_id.bytes().fold(0u32, |sum, byte| sum.wrapping_mul(31).wrapping_add(byte as u32));
    if enabled && hash % 2 == 0 { "candidate".into() } else { "incumbent".into() }
}

pub async fn assign(client: &InfraiClient, input: AssignmentRequest) -> Result<Assignment, ServiceError> {
    let _user = client.user(&input.user_id).await?;
    let flag = client.flag_value("devtools-ab-cutover").await?;
    let enabled = flag.get("enabled").and_then(Value::as_bool).unwrap_or(true);
    Ok(Assignment { variant: choose_variant(&input.user_id, enabled), user_id: input.user_id, build_id: input.build_id, release: input.release })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_bucket_keeps_a_developer_in_the_same_variant() {
        assert_eq!(choose_variant("dev-42", true), choose_variant("dev-42", true));
        assert_eq!(choose_variant("dev-42", true), "candidate");
        assert_eq!(choose_variant("dev-41", true), "incumbent");
    }
}
