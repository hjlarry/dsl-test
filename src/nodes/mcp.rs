use crate::memory::{GlobalMemory, NodeMemory, NodeOutput};
use crate::nodes::NodeExecutor;
use crate::schema::Node;
use crate::template::TemplateEngine;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

pub struct McpExecutor;

#[derive(Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<u64>,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

#[derive(Serialize, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<u64>,
    result: Option<Value>,
    error: Option<Value>,
}

#[async_trait]
impl NodeExecutor for McpExecutor {
    async fn execute(
        &self,
        node: &Node,
        global: &GlobalMemory,
        nodes: &NodeMemory,
    ) -> Result<NodeOutput> {
        let template = TemplateEngine::new(global.clone(), nodes.clone());

        // Parse parameters
        let server_config = node.params.get("server").context("Missing 'server' param")?;
        let command_str = server_config
            .get("command")
            .and_then(|v| v.as_str())
            .context("Missing 'server.command'")?;
        
        let args: Vec<String> = server_config
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let tool_name = node.params.get("tool").and_then(|v| v.as_str()).context("Missing 'tool' param")?;
        let raw_tool_args = node.params.get("arguments").cloned().unwrap_or(json!({}));

        // Render tool arguments
        let tool_args = render_value(&template, &raw_tool_args)?;

        // Spawn server process
        let mut child = Command::new(command_str)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .context("Failed to spawn MCP server")?;

        let mut stdin = child.stdin.take().context("Failed to open stdin")?;
        let stdout = child.stdout.take().context("Failed to open stdout")?;
        let mut reader = BufReader::new(stdout).lines();

        // 1. Initialize
        let init_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(1),
            method: "initialize".to_string(),
            params: Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "workflow-engine",
                    "version": "0.1.0"
                }
            })),
        };
        
        let init_json = serde_json::to_string(&init_req)?;
        stdin.write_all(init_json.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;

        // Read initialize response
        let line = reader.next_line().await?.context("Server closed connection during init")?;
        let init_resp: JsonRpcResponse = serde_json::from_str(&line)?;
        
        if let Some(err) = init_resp.error {
            return Ok(NodeOutput {
                status: "failed".to_string(),
                output: json!({ "error": "Initialize failed", "details": err }),
            });
        }

        // 2. Initialized notification
        let initialized_notif = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "notifications/initialized".to_string(),
            params: None,
        };
        stdin.write_all(serde_json::to_string(&initialized_notif)?.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;

        // 3. Call Tool
        let call_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(2),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": tool_name,
                "arguments": tool_args
            })),
        };
        
        stdin.write_all(serde_json::to_string(&call_req)?.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;

        // Read tool response
        let line = reader.next_line().await?.context("Server closed connection during tool call")?;
        let call_resp: JsonRpcResponse = serde_json::from_str(&line)?;

        // Cleanup
        drop(stdin); // Close stdin to signal EOF
        child.kill().await.ok(); // Ensure process is dead

        if let Some(err) = call_resp.error {
            Ok(NodeOutput {
                status: "failed".to_string(),
                output: json!({ "error": "Tool call failed", "details": err }),
            })
        } else if let Some(result) = call_resp.result {
            Ok(NodeOutput {
                status: "success".to_string(),
                output: result,
            })
        } else {
             Ok(NodeOutput {
                status: "failed".to_string(),
                output: json!({ "error": "Empty response" }),
            })
        }
    }
}

fn render_value(template: &TemplateEngine, value: &Value) -> Result<Value> {
    match value {
        Value::String(s) => {
            let rendered = template.render(s)?;
            // Try to parse as JSON if it looks like JSON, otherwise keep as string
            if (rendered.starts_with('{') && rendered.ends_with('}')) || 
               (rendered.starts_with('[') && rendered.ends_with(']')) {
                if let Ok(parsed) = serde_json::from_str(&rendered) {
                    return Ok(parsed);
                }
            }
            Ok(Value::String(rendered))
        },
        Value::Array(arr) => {
            let mut new_arr = Vec::new();
            for v in arr {
                new_arr.push(render_value(template, v)?);
            }
            Ok(Value::Array(new_arr))
        },
        Value::Object(obj) => {
            let mut new_obj = serde_json::Map::new();
            for (k, v) in obj {
                new_obj.insert(k.clone(), render_value(template, v)?);
            }
            Ok(Value::Object(new_obj))
        },
        _ => Ok(value.clone()),
    }
}
