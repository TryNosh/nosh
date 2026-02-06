use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::context::ConversationContext;

#[derive(Deserialize)]
pub struct Usage {
    pub subscription_balance: i32,
    pub pack_balance: i32,
    pub total_balance: i32,
    pub tokens_used: i32,
    pub monthly_allowance: i32,
    #[allow(dead_code)]
    pub period_start: Option<String>,
    pub resets_at: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct PlanInfo {
    pub plan: Option<String>,
    #[allow(dead_code)]
    pub status: Option<String>,
    #[allow(dead_code)]
    pub current_period_end: Option<String>,
    pub cancel_at_period_end: bool,
}

#[derive(Serialize)]
struct ContextExchange {
    user_input: String,
    ai_command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_summary: Option<String>,
}

#[derive(Serialize)]
struct CompleteRequest {
    input: String,
    cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<Vec<ContextExchange>>,
}

#[derive(Deserialize)]
struct CompleteResponse {
    command: String,
    #[allow(dead_code)]
    tokens_used: Option<i32>,
    tokens_remaining: Option<i32>,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
    #[allow(dead_code)]
    code: Option<String>,
    message: Option<String>,
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

    pub async fn translate(
        &self,
        input: &str,
        cwd: &str,
        context: Option<&ConversationContext>,
    ) -> Result<(String, i32)> {
        // Convert context to API format
        let context_exchanges = context.filter(|c| !c.is_empty()).map(|c| {
            c.exchanges()
                .map(|e| ContextExchange {
                    user_input: e.user_input.clone(),
                    ai_command: e.ai_command.clone(),
                    output_summary: e.output_summary.clone(),
                })
                .collect()
        });

        let request = CompleteRequest {
            input: input.to_string(),
            cwd: cwd.to_string(),
            context: context_exchanges,
        };

        let response = self
            .client
            .post(format!("{}/ai/complete", self.base_url))
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&request)
            .send()
            .await?;

        if response.status() == 402 {
            let error: ErrorResponse = response.json().await.unwrap_or(ErrorResponse {
                error: "Out of tokens".to_string(),
                code: None,
                message: Some("Run /buy to get more tokens".to_string()),
            });
            let msg = error.message.unwrap_or_else(|| "Run /buy to get more tokens".to_string());
            return Err(anyhow!("Out of tokens. {}", msg));
        }

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            return Err(anyhow!("Cloud error: {}", error.error));
        }

        let result: CompleteResponse = response.json().await?;
        Ok((result.command, result.tokens_remaining.unwrap_or(0)))
    }

    pub async fn get_usage(&self) -> Result<Usage> {
        let response = self
            .client
            .get(format!("{}/account/tokens", self.base_url))
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get token balance"));
        }

        let result: Usage = response.json().await?;
        Ok(result)
    }

    pub async fn buy_tokens(&self) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/billing/buy-tokens", self.base_url))
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&serde_json::json!({ "quantity": 1 }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            return Err(anyhow!("Failed to get checkout URL: {}", error.error));
        }

        #[derive(Deserialize)]
        struct CheckoutResponse {
            url: String,
        }

        let result: CheckoutResponse = response.json().await?;
        Ok(result.url)
    }

    pub async fn subscribe(&self, plan: &str) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/billing/subscribe", self.base_url))
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&serde_json::json!({ "plan": plan }))
            .send()
            .await?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            return Err(anyhow!("{}", error.error));
        }

        #[derive(Deserialize)]
        struct CheckoutResponse {
            url: String,
        }

        let result: CheckoutResponse = response.json().await?;
        Ok(result.url)
    }

    pub async fn get_plan(&self) -> Result<PlanInfo> {
        let response = self
            .client
            .get(format!("{}/account/plan", self.base_url))
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to get plan info"));
        }

        let result: PlanInfo = response.json().await?;
        Ok(result)
    }

    pub async fn cancel_subscription(&self) -> Result<()> {
        let response = self
            .client
            .post(format!("{}/billing/cancel", self.base_url))
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            return Err(anyhow!("{}", error.error));
        }

        Ok(())
    }

    pub async fn get_portal_url(&self) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/billing/portal", self.base_url))
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error: ErrorResponse = response.json().await?;
            return Err(anyhow!("{}", error.error));
        }

        #[derive(Deserialize)]
        struct PortalResponse {
            url: String,
        }

        let result: PortalResponse = response.json().await?;
        Ok(result.url)
    }
}
