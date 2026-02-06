use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    stream: bool,
    system: String,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
}

pub struct OllamaClient {
    client: Client,
    base_url: String,
    model: String,
}

impl OllamaClient {
    pub fn new(model: &str, base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
            model: model.to_string(),
        }
    }

    pub async fn translate(&self, input: &str, cwd: &str) -> Result<String> {
        let system_prompt = format!(
            r#"You are a shell command translator. Convert natural language to shell commands.

Current directory: {}

Rules:
1. Output ONLY the shell command, nothing else
2. No explanations, no markdown, no code blocks
3. If the input is already a valid shell command, output it unchanged
4. Use common Unix commands (ls, grep, find, etc.)
5. For dangerous operations (rm, sudo), still output the command - safety is handled separately

Examples:
- "list all files" -> ls -la
- "show disk usage" -> df -h
- "find all rust files" -> find . -name "*.rs"
- "git status" -> git status"#,
            cwd
        );

        let request = GenerateRequest {
            model: self.model.clone(),
            prompt: input.to_string(),
            stream: false,
            system: system_prompt,
        };

        let response = self
            .client
            .post(format!("{}/api/generate", self.base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Ollama request failed: {}",
                response.status()
            ));
        }

        let result: GenerateResponse = response.json().await?;
        Ok(result.response.trim().to_string())
    }

    pub async fn check_available(&self) -> bool {
        self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .is_ok()
    }
}
