use crate::nodes::NodeExecutor;
use crate::memory::{NodeOutput, GlobalMemory, NodeMemory};
use crate::schema::Node;
use crate::template::TemplateEngine;
use anyhow::{Result, Context};
use async_trait::async_trait;
use std::process::Stdio;
use tokio::process::Command;

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
