//! Pipefish/Ollama-compatible chat client for LLM-backed LSP features.
//!
//! Ported unchanged from `exosphere/crates/ai/services/lsp/src/ollama.rs` — this
//! file was already real and working, unlike the rest of that crate's "AI"
//! layer. Defaults to the fleet's live `pipefish` service (127.0.0.1:11450).

use anyhow::Result;

pub struct LlmClient {
    client: reqwest::Client,
    base_url: String,
    model: String,
}

impl LlmClient {
    pub fn new() -> Self {
        let base_url = std::env::var("PIPEFISH_URL")
            .or_else(|_| std::env::var("OLLAMA_HOST"))
            .unwrap_or_else(|_| "http://127.0.0.1:11450".to_string());

        let model =
            std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2-1b.Q8_0".to_string());

        Self {
            client: reqwest::Client::new(),
            base_url,
            model,
        }
    }

    pub async fn chat(&self, prompt: &str, system_prompt: Option<&str>) -> Result<String> {
        let mut messages = Vec::new();

        if let Some(sys) = system_prompt {
            messages.push(serde_json::json!({"role": "system", "content": sys}));
        }
        messages.push(serde_json::json!({"role": "user", "content": prompt}));

        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "stream": false,
        });

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Pipefish request failed: {}", response.status());
        }

        let data: serde_json::Value = response.json().await?;
        Ok(data["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }

    pub async fn complete(&self, code: &str, cursor_context: &str) -> Result<String> {
        let prompt = format!(
            "You are a code completion assistant for a language called Crush. \
            Complete the following code at the cursor position.\n\n\
            Code:\n{}\n\n\
            Cursor context: {}\n\n\
            Provide only the completion, no explanations.",
            code, cursor_context
        );
        self.chat(&prompt, Some("You are a helpful code completion assistant."))
            .await
    }

    pub async fn explain(&self, code: &str, line: u32) -> Result<String> {
        let prompt = format!(
            "Explain what the code at line {} does:\n\n```\n{}\n```",
            line, code
        );
        self.chat(&prompt, Some("You are a helpful programming assistant."))
            .await
    }

    pub async fn fix_suggestion(&self, code: &str, error: &str) -> Result<String> {
        let prompt = format!(
            "This code has an error:\n\n```\n{}\n```\n\nError: {}\n\n\
            Provide a fixed version of the code.",
            code, error
        );
        self.chat(&prompt, Some("You are a helpful programming assistant."))
            .await
    }
}

impl Default for LlmClient {
    fn default() -> Self {
        Self::new()
    }
}
