use crate::nodes::NodeExecutor;
use crate::memory::{NodeOutput, GlobalMemory, NodeMemory};
use crate::schema::Node;
use crate::template::TemplateEngine;
use anyhow::{Result, Context};
use async_trait::async_trait;
use std::process::Stdio;
use tokio::process::Command;
use uuid;

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
