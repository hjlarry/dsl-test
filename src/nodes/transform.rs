use crate::nodes::NodeExecutor;
use crate::memory::{NodeOutput, GlobalMemory, NodeMemory};
use crate::schema::Node;
use crate::template::TemplateEngine;
use anyhow::{Result, Context};
use async_trait::async_trait;
use serde_json::Value;

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
