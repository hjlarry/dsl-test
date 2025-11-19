use crate::nodes::NodeExecutor;
use crate::memory::{NodeOutput, GlobalMemory, NodeMemory};
use crate::schema::Node;
use crate::template::TemplateEngine;
use anyhow::{Result, Context};
use async_trait::async_trait;
use serde_json::Value;
use crate::engine::Engine;
use crate::schema::Workflow;

pub struct LoopExecutor;

#[async_trait]
impl NodeExecutor for LoopExecutor {
    async fn execute(
        &self,
        node: &Node,
        global: &GlobalMemory,
        nodes: &NodeMemory,
    ) -> Result<NodeOutput> {
        let template = TemplateEngine::new(global.clone(), nodes.clone());
        
        // 1. Get items to iterate
        let items_param = node.params
            .get("items")
            .context("Loop node requires 'items' parameter")?;
            
        // If items is a string (template), render and parse it
        let items: Vec<Value> = if let Some(s) = items_param.as_str() {
            let rendered = template.render(s)?;
            serde_json::from_str(&rendered)
                .or_else(|_| {
                    // If not JSON, maybe it's just a string we want to treat as a single item list?
                    // Or maybe it failed to parse. Let's try to see if it's a JSON array.
                    anyhow::bail!("Failed to parse 'items' as JSON array: {}", rendered)
                })?
        } else if let Some(arr) = items_param.as_array() {
            arr.clone()
        } else {
            anyhow::bail!("'items' parameter must be an array")
        };

        // 2. Get steps (sub-workflow nodes)
        let steps_val = node.params
            .get("steps")
            .context("Loop node requires 'steps' parameter")?;
            
        let steps: Vec<Node> = serde_json::from_value(steps_val.clone())
            .context("Failed to parse 'steps' as list of Nodes")?;

        log::info!("Looping over {} items with {} steps", items.len(), steps.len());

        let mut results = Vec::new();

        // 3. Iterate
        for (index, item) in items.iter().enumerate() {
            log::info!("Loop iteration {}/{}", index + 1, items.len());

            // Create a sub-workflow
            let sub_workflow = Workflow {
                name: format!("{}_iter_{}", node.name, index),
                version: "1.0".to_string(),
                global: std::collections::HashMap::new(), // We'll inject global memory manually
                nodes: steps.clone(),
            };

            // Use the SAME global memory to allow state sharing and accumulation across iterations.
            // We clone the Arc, so it points to the same DashMap.
            let iter_global = global.clone();
            
            // Inject loop context
            let loop_ctx = serde_json::json!({
                "index": index,
                "item": item,
                "total": items.len()
            });
            iter_global.set("loop".to_string(), loop_ctx);

            let engine = Engine::new_with_memory(sub_workflow, iter_global);
            
            // Execute sub-workflow
            engine.execute().await?;
            
            // Collect outputs from this iteration
            // We might want to return the output of the LAST node, or a map of all nodes?
            // Let's return a map of all node outputs for this iteration.
            let node_outputs: std::collections::HashMap<String, Value> = engine.get_node_memory().get_all_values();
            results.push(serde_json::json!(node_outputs));
        }

        Ok(NodeOutput {
            status: "success".to_string(),
            output: serde_json::json!({
                "results": results
            }),
        })
    }
}
