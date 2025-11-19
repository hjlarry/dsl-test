use crate::nodes::NodeExecutor;
use crate::memory::{NodeOutput, GlobalMemory, NodeMemory};
use crate::schema::Node;
use crate::template::TemplateEngine;
use anyhow::{Result, Context};
use async_trait::async_trait;
use serde_json::Value;

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
