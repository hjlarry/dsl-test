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
        "llm" => Ok(Box::new(LlmExecutor)),
        "transform" => Ok(Box::new(TransformExecutor)),
        "file" => Ok(Box::new(FileExecutor)),
        _ => anyhow::bail!("Unknown node type: {}", node_type),
    }
}

// ==================== LLM Executor ====================

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

// ==================== Transform Executor ====================

pub struct TransformExecutor;

#[async_trait]
impl NodeExecutor for TransformExecutor {
    async fn execute(
        &self,
        node: &Node,
        global: &GlobalMemory,
        nodes: &NodeMemory,
    ) -> Result<NodeOutput> {
        let template = TemplateEngine::new(global.clone(), nodes.clone());
        
        let input = node.params
            .get("input")
            .context("Transform node requires 'input' parameter")?;

        // Render input if it's a string
        let input_value: Value = if let Some(input_str) = input.as_str() {
            let rendered = template.render(input_str)?;
            serde_json::from_str(&rendered)
                .unwrap_or_else(|_| Value::String(rendered))
        } else {
            input.clone()
        };

        log::info!("Transforming data with JSONPath");

        // Single path extraction
        if let Some(path) = node.params.get("path").and_then(|v| v.as_str()) {
            let mut selector = jsonpath_lib::selector(&input_value);
            let result_vec = selector(path)
                .context(format!("JSONPath '{}' evaluation failed", path))?;
            
            // Convert Vec<&Value> to Value
            let result = Value::Array(result_vec.into_iter().cloned().collect());
            
            return Ok(NodeOutput {
                status: "success".to_string(),
                output: serde_json::json!({"result": result}),
            });
        }

        // Multiple field extraction
        if let Some(extract_obj) = node.params.get("extract").and_then(|v| v.as_object()) {
            let mut result = serde_json::Map::new();
            
            for (key, path_value) in extract_obj {
                if let Some(path) = path_value.as_str() {
                    let mut selector = jsonpath_lib::selector(&input_value);
                    let extracted_vec = selector(path)
                        .context(format!("JSONPath '{}' evaluation failed", path))?;
                    
                    // Convert Vec<&Value> to Value
                    let extracted = Value::Array(extracted_vec.into_iter().cloned().collect());
                    result.insert(key.clone(), extracted);
                }
            }
            
            return Ok(NodeOutput {
                status: "success".to_string(),
                output: Value::Object(result),
            });
        }

        anyhow::bail!("Transform node requires either 'path' or 'extract' parameter")
    }
}

// ==================== File Executor ====================

pub struct FileExecutor;

#[async_trait]
impl NodeExecutor for FileExecutor {
    async fn execute(
        &self,
        node: &Node,
        global: &GlobalMemory,
        nodes: &NodeMemory,
    ) -> Result<NodeOutput> {
        let template = TemplateEngine::new(global.clone(), nodes.clone());
        
        let operation = node.params
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("read");

        let path = node.params
            .get("path")
            .and_then(|v| v.as_str())
            .context("File node requires 'path' parameter")?;

        let rendered_path = template.render(path)?;

        log::info!("File operation: {} on {}", operation, rendered_path);

        match operation {
            "read" => {
                let content = tokio::fs::read_to_string(&rendered_path)
                    .await
                    .context(format!("Failed to read file: {}", rendered_path))?;

                Ok(NodeOutput {
                    status: "success".to_string(),
                    output: serde_json::json!({
                        "content": content,
                        "path": rendered_path
                    }),
                })
            }
            "write" | "append" => {
                let content = node.params
                    .get("content")
                    .context("File write/append requires 'content' parameter")?;

                let content_str = match content {
                    Value::String(s) => template.render(s)?,
                    _ => content.to_string(),
                };

                if operation == "write" {
                    tokio::fs::write(&rendered_path, content_str.as_bytes())
                        .await
                        .context(format!("Failed to write file: {}", rendered_path))?;
                } else {
                    use tokio::io::AsyncWriteExt;
                    let mut file = tokio::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&rendered_path)
                        .await
                        .context(format!("Failed to open file for append: {}", rendered_path))?;
                    
                    file.write_all(content_str.as_bytes())
                        .await
                        .context("Failed to append to file")?;
                }

                Ok(NodeOutput {
                    status: "success".to_string(),
                    output: serde_json::json!({
                        "path": rendered_path,
                        "operation": operation,
                        "bytes_written": content_str.len()
                    }),
                })
            }
            _ => anyhow::bail!("Unsupported file operation: {}", operation),
        }
    }
}
