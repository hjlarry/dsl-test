use crate::nodes::NodeExecutor;
use crate::memory::{NodeOutput, GlobalMemory, NodeMemory};
use crate::schema::Node;
use crate::template::TemplateEngine;
use anyhow::{Result, Context};
use async_trait::async_trait;
use serde_json::Value;

pub struct InputExecutor;

#[async_trait]
impl NodeExecutor for InputExecutor {
    async fn execute(
        &self,
        node: &Node,
        global: &GlobalMemory,
        nodes: &NodeMemory,
    ) -> Result<NodeOutput> {
        let template = TemplateEngine::new(global.clone(), nodes.clone());
        
        let prompt = node.params
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("Please enter value:");
            
        let rendered_prompt = template.render(prompt)?;
        
        let default = node.params
            .get("default")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Use tokio's blocking task for stdin interaction to avoid blocking the runtime
        let result = tokio::task::spawn_blocking(move || {
            use std::io::{self, Write};
            
            print!("{} ", rendered_prompt);
            if let Some(def) = &default {
                print!("[default: {}] ", def);
            }
            io::stdout().flush().context("Failed to flush stdout")?;

            let mut input = String::new();
            io::stdin().read_line(&mut input).context("Failed to read line")?;
            
            let trimmed = input.trim();
            if trimmed.is_empty() {
                if let Some(def) = default {
                    return Ok(def);
                }
            }
            
            Ok::<String, anyhow::Error>(trimmed.to_string())
        }).await??;

        Ok(NodeOutput {
            status: "success".to_string(),
            output: Value::String(result),
        })
    }
}
