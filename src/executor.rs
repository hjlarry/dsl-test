use crate::memory::{NodeOutput, GlobalMemory, NodeMemory};
use crate::schema::Node;
use crate::template::TemplateEngine;
use anyhow::{Result, Context};
use async_trait::async_trait;
use serde_json::Value;
use std::process::Stdio;
use tokio::process::Command;
use uuid;

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

pub struct SwitchExecutor;

#[async_trait]
impl NodeExecutor for SwitchExecutor {
    async fn execute(
        &self,
        node: &Node,
        global: &GlobalMemory,
        nodes: &NodeMemory,
    ) -> Result<NodeOutput> {
        let template = TemplateEngine::new(global.clone(), nodes.clone());
        
        let condition = node.params
            .get("condition")
            .and_then(|v| v.as_str())
            .context("Switch node requires 'condition' parameter")?;

        let rendered_condition = template.render(condition)?;
        
        log::info!("Evaluating condition: {}", rendered_condition);

        // Simple boolean evaluation
        let result = evaluate_condition(&rendered_condition)?;
        
        let value = node.params.get(if result { "true_value" } else { "false_value" });
        
        let output_value = match value {
            Some(v) if v.is_string() => {
                let s = v.as_str().unwrap();
                Value::String(template.render(s)?)
            }
            Some(v) => v.clone(),
            None => Value::Bool(result),
        };

        Ok(NodeOutput {
            status: "success".to_string(),
            output: serde_json::json!({
                "condition": rendered_condition,
                "result": result,
                "value": output_value
            }),
        })
    }
}

/// Simple condition evaluator supporting basic comparisons
fn evaluate_condition(expr: &str) -> Result<bool> {
    let expr = expr.trim();
    
    // Boolean literals
    if expr == "true" {
        return Ok(true);
    }
    if expr == "false" {
        return Ok(false);
    }
    
    // Numeric comparisons: ==, !=, >, <, >=, <=
    if let Some(pos) = expr.find("==") {
        let left = expr[..pos].trim();
        let right = expr[pos+2..].trim();
        return Ok(left == right);
    }
    
    if let Some(pos) = expr.find("!=") {
        let left = expr[..pos].trim();
        let right = expr[pos+2..].trim();
        return Ok(left != right);
    }
    
    if let Some(pos) = expr.find(">=") {
        let left = parse_number(expr[..pos].trim())?;
        let right = parse_number(expr[pos+2..].trim())?;
        return Ok(left >= right);
    }
    
    if let Some(pos) = expr.find("<=") {
        let left = parse_number(expr[..pos].trim())?;
        let right = parse_number(expr[pos+2..].trim())?;
        return Ok(left <= right);
    }
    
    if let Some(pos) = expr.find('>') {
        let left = parse_number(expr[..pos].trim())?;
        let right = parse_number(expr[pos+1..].trim())?;
        return Ok(left > right);
    }
    
    if let Some(pos) = expr.find('<') {
        let left = parse_number(expr[..pos].trim())?;
        let right = parse_number(expr[pos+1..].trim())?;
        return Ok(left < right);
    }
    
    // If no operator found, try to parse as boolean
    anyhow::bail!("Invalid condition expression: {}", expr)
}

fn parse_number(s: &str) -> Result<f64> {
    s.parse::<f64>()
        .with_context(|| format!("Cannot parse '{}' as number", s))
}

pub struct ScriptExecutor;

#[async_trait]
impl NodeExecutor for ScriptExecutor {
    async fn execute(
        &self,
        node: &Node,
        global: &GlobalMemory,
        nodes: &NodeMemory,
    ) -> Result<NodeOutput> {
        let template = TemplateEngine::new(global.clone(), nodes.clone());
        
        let script = node.params
            .get("script")
            .and_then(|v| v.as_str())
            .context("Script node requires 'script' parameter")?;

        let language = node.params
            .get("language")
            .and_then(|v| v.as_str())
            .unwrap_or("python");

        let rendered_script = template.render(script)?;
        
        log::info!("Executing {} script", language);

        let output = match language {
            "python" | "python3" => execute_python(&rendered_script).await?,
            "javascript" | "js" | "node" => execute_javascript(&rendered_script).await?,
            _ => anyhow::bail!("Unsupported script language: {}", language),
        };

        Ok(output)
    }
}

async fn execute_python(script: &str) -> Result<NodeOutput> {
    // Create a temporary file for the script
    let temp_file = std::env::temp_dir().join(format!("workflow_script_{}.py", uuid::Uuid::new_v4()));
    tokio::fs::write(&temp_file, script).await
        .context("Failed to write Python script to temp file")?;

    let output = Command::new("python3")
        .arg(&temp_file)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to execute Python script. Is python3 installed?")?;

    // Clean up temp file
    let _ = tokio::fs::remove_file(&temp_file).await;

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

async fn execute_javascript(script: &str) -> Result<NodeOutput> {
    // Create a temporary file for the script
    let temp_file = std::env::temp_dir().join(format!("workflow_script_{}.js", uuid::Uuid::new_v4()));
    tokio::fs::write(&temp_file, script).await
        .context("Failed to write JavaScript script to temp file")?;

    let output = Command::new("node")
        .arg(&temp_file)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to execute JavaScript script. Is node installed?")?;

    // Clean up temp file
    let _ = tokio::fs::remove_file(&temp_file).await;

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

pub fn get_executor(node_type: &str) -> Result<Box<dyn NodeExecutor>> {
    match node_type {
        "shell" => Ok(Box::new(ShellExecutor)),
        "http" => Ok(Box::new(HttpExecutor)),
        "delay" => Ok(Box::new(DelayExecutor)),
        "switch" => Ok(Box::new(SwitchExecutor)),
        "script" => Ok(Box::new(ScriptExecutor)),
        _ => anyhow::bail!("Unknown node type: {}", node_type),
    }
}
