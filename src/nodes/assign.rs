use crate::nodes::NodeExecutor;
use crate::memory::{NodeOutput, GlobalMemory, NodeMemory};
use crate::schema::Node;
use crate::template::TemplateEngine;
use anyhow::{Result, Context};
use async_trait::async_trait;
use serde_json::Value;

pub struct AssignExecutor;

#[async_trait]
impl NodeExecutor for AssignExecutor {
    async fn execute(
        &self,
        node: &Node,
        global: &GlobalMemory,
        nodes: &NodeMemory,
    ) -> Result<NodeOutput> {
        let template = TemplateEngine::new(global.clone(), nodes.clone());
        
        // Parse "assignments" parameter
        // Format:
        // assignments:
        //   - key: "variable_name"
        //     value: "some value"
        //     mode: "set" | "append" (optional, default "set")
        
        let assignments_val = node.params
            .get("assignments")
            .context("Assign node requires 'assignments' parameter")?;
            
        let assignments = assignments_val.as_array()
            .context("'assignments' must be an array")?;

        let mut output_map = serde_json::Map::new();

        for assign in assignments {
            let key = assign.get("key")
                .and_then(|v| v.as_str())
                .context("Assignment requires 'key'")?;
                
            let value_template = assign.get("value")
                .context("Assignment requires 'value'")?;
            
            let mode = assign.get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("set");

            // Render value
            let rendered_value = if let Some(s) = value_template.as_str() {
                let rendered = template.render(s)?;
                // Try to parse as JSON, otherwise keep as string
                serde_json::from_str(&rendered)
                    .unwrap_or(Value::String(rendered))
            } else {
                value_template.clone()
            };

            match mode {
                "set" => {
                    global.set(key.to_string(), rendered_value.clone());
                    output_map.insert(key.to_string(), rendered_value);
                }
                "append" => {
                    // Get existing list or create new
                    let mut list = global.get(key).unwrap_or(Value::Array(vec![]));
                    if let Value::Array(ref mut arr) = list {
                        arr.push(rendered_value.clone());
                        global.set(key.to_string(), list.clone());
                        output_map.insert(key.to_string(), list);
                    } else {
                        log::warn!("Cannot append to non-array variable '{}'", key);
                    }
                }
                _ => {
                    log::warn!("Unknown assignment mode '{}', skipping", mode);
                }
            }
        }

        Ok(NodeOutput {
            status: "success".to_string(),
            output: Value::Object(output_map),
        })
    }
}
