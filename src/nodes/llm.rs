use crate::nodes::NodeExecutor;
use crate::memory::{NodeOutput, GlobalMemory, NodeMemory};
use crate::schema::Node;
use crate::template::TemplateEngine;
use anyhow::{Result, Context};
use async_trait::async_trait;
use serde_json::Value;

pub struct LlmExecutor;

#[async_trait]
impl NodeExecutor for LlmExecutor {
    async fn execute(
        &self,
        node: &Node,
        global: &GlobalMemory,
        nodes: &NodeMemory,
    ) -> Result<NodeOutput> {
        let template = TemplateEngine::new(global.clone(), nodes.clone());
        
        // Get API key from environment or params
        let api_key = std::env::var("OPENAI_API_KEY")
            .or_else(|_| {
                node.params
                    .get("api_key")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| anyhow::anyhow!("OPENAI_API_KEY not found in environment or params"))
            })?;

        let base_url = node.params
            .get("base_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| std::env::var("OPENAI_BASE_URL").ok())
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

        let model = node.params
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("gpt-3.5-turbo");

        let system = node.params
            .get("system")
            .and_then(|v| v.as_str())
            .map(|s| template.render(s))
            .transpose()?;

        let prompt = node.params
            .get("prompt")
            .and_then(|v| v.as_str())
            .context("LLM node requires 'prompt' parameter")?;
        
        let rendered_prompt = template.render(prompt)?;

        let temperature = node.params
            .get("temperature")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.7);

        let max_tokens = node.params
            .get("max_tokens")
            .and_then(|v| v.as_u64())
            .map(|v| v as i32);

        log::info!("Calling LLM: {} (model: {})", node.name, model);

        // Build messages
        let mut messages = vec![];
        if let Some(sys) = system {
            messages.push(serde_json::json!({
                "role": "system",
                "content": sys
            }));
        }
        messages.push(serde_json::json!({
            "role": "user",
            "content": rendered_prompt
        }));

        // Build request body
        let mut request_body = serde_json::json!({
            "model": model,
            "messages": messages,
            "temperature": temperature
        });

        if let Some(tokens) = max_tokens {
            request_body["max_tokens"] = serde_json::json!(tokens);
        }

        // Call OpenAI API
        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/chat/completions", base_url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to call LLM API")?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            anyhow::bail!("LLM API error ({}): {}", status, response_text);
        }

        let response_json: Value = serde_json::from_str(&response_text)
            .context("Failed to parse LLM response")?;

        let content = response_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let usage = response_json.get("usage").cloned().unwrap_or(Value::Null);

        let result = serde_json::json!({
            "content": content,
            "model": model,
            "usage": usage
        });

        Ok(NodeOutput {
            status: "success".to_string(),
            output: result,
        })
    }
}
