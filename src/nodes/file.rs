use crate::nodes::NodeExecutor;
use crate::memory::{NodeOutput, GlobalMemory, NodeMemory};
use crate::schema::Node;
use crate::template::TemplateEngine;
use anyhow::{Result, Context};
use async_trait::async_trait;
use serde_json::Value;

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
