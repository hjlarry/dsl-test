use crate::memory::{NodeOutput, GlobalMemory, NodeMemory};
use crate::schema::Node;
use crate::template::TemplateEngine;
use anyhow::{Result, Context};
use async_trait::async_trait;
use serde_json::Value;
use std::process::Stdio;
use tokio::process::Command;

#[async_trait]
pub trait NodeExecutor: Send + Sync {
    async fn execute(
        &self,
        node: &Node,
        global: &GlobalMemory,
        nodes: &NodeMemory,
    ) -> Result<NodeOutput>;
}

pub struct ShellExecutor;

#[async_trait]
impl NodeExecutor for ShellExecutor {
    async fn execute(
        &self,
        node: &Node,
        global: &GlobalMemory,
        nodes: &NodeMemory,
    ) -> Result<NodeOutput> {
        let template = TemplateEngine::new(global.clone(), nodes.clone());
        
        let command = node.params
            .get("command")
            .and_then(|v| v.as_str())
            .context("Shell node requires 'command' parameter")?;

        let rendered_command = template.render(command)?;
        
        log::info!("Executing shell command: {}", rendered_command);

        let output = Command::new("sh")
            .arg("-c")
            .arg(&rendered_command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Failed to execute shell command")?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let success = output.status.success();

        let result = serde_json::json!({
            "stdout": stdout.trim(),
            "stderr": stderr.trim(),
            "exit_code": output.status.code().unwrap_or(-1),
            "success": success
        });

        Ok(NodeOutput {
            status: if success { "success".to_string() } else { "failed".to_string() },
            output: result,
        })
    }
}

pub struct HttpExecutor;

#[async_trait]
impl NodeExecutor for HttpExecutor {
    async fn execute(
        &self,
        node: &Node,
        global: &GlobalMemory,
        nodes: &NodeMemory,
    ) -> Result<NodeOutput> {
        let template = TemplateEngine::new(global.clone(), nodes.clone());
        
        let url = node.params
            .get("url")
            .and_then(|v| v.as_str())
            .context("HTTP node requires 'url' parameter")?;

        let method = node.params
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET");

        let rendered_url = template.render(url)?;
        
        log::info!("HTTP {} request to: {}", method, rendered_url);

        let client = reqwest::Client::new();
        let response = match method.to_uppercase().as_str() {
            "GET" => client.get(&rendered_url).send().await?,
            "POST" => {
                let body = node.params.get("body").unwrap_or(&Value::Null);
                client.post(&rendered_url).json(&body).send().await?
            }
            _ => anyhow::bail!("Unsupported HTTP method: {}", method),
        };

        let status = response.status().as_u16();
        let body = response.text().await?;

        let result = serde_json::json!({
            "status": status,
            "body": body,
            "success": status >= 200 && status < 300
        });

        Ok(NodeOutput {
            status: "success".to_string(),
            output: result,
        })
    }
}

pub struct DelayExecutor;

#[async_trait]
impl NodeExecutor for DelayExecutor {
    async fn execute(
        &self,
        node: &Node,
        _global: &GlobalMemory,
        _nodes: &NodeMemory,
    ) -> Result<NodeOutput> {
        let ms = node.params
            .get("milliseconds")
            .and_then(|v| v.as_u64())
            .context("Delay node requires 'milliseconds' parameter")?;

        log::info!("Delaying for {} ms", ms);
        tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;

        Ok(NodeOutput {
            status: "success".to_string(),
            output: Value::String(format!("Delayed for {} ms", ms)),
        })
    }
}

pub fn get_executor(node_type: &str) -> Result<Box<dyn NodeExecutor>> {
    match node_type {
        "shell" => Ok(Box::new(ShellExecutor)),
        "http" => Ok(Box::new(HttpExecutor)),
        "delay" => Ok(Box::new(DelayExecutor)),
        _ => anyhow::bail!("Unknown node type: {}", node_type),
    }
}
