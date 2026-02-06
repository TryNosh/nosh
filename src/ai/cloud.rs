use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct CompleteRequest {
    input: String,
    cwd: String,
}

#[derive(Deserialize)]
struct CompleteResponse {
    command: String,
    credits_remaining: i32,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
    #[allow(dead_code)]
    code: Option<String>,
}

pub struct CloudClient {
    client: Client,
    base_url: String,
    token: String,
}

impl CloudClient {
    pub fn new(token: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: std::env::var("NOSH_CLOUD_URL")
                .unwrap_or_else(|_| "https://nosh.sh/api".to_string()),
            token: token.to_string(),
        }
    }

    pub async fn translate(&self, input: &str, cwd: &str) -> Result<(String, i32)> {
        let request = CompleteRequest {
            input: input.to_string(),
            cwd: cwd.to_string(),
        };

        let response = self
            .client
            .post(format!("{}/ai/complete", self.base_url))
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&request)
            .send()
            .await?;

        if response.status() == 402 {
            return Err(anyhow!("Out of credits"));
        }

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            return Err(anyhow!("Cloud error: {}", error.error));
        }

        let result: CompleteResponse = response.json().await?;
        Ok((result.command, result.credits_remaining))
    }

    pub async fn get_credits(&self) -> Result<i32> {
        let response = self
            .client
            .get(format!("{}/account/credits", self.base_url))
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get credits"));
        }

        #[derive(Deserialize)]
        struct CreditsResponse {
            balance: i32,
        }

        let result: CreditsResponse = response.json().await?;
        Ok(result.balance)
    }
}
