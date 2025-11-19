use crate::nodes::NodeExecutor;
use crate::memory::{NodeOutput, GlobalMemory, NodeMemory};
use crate::schema::Node;
use crate::template::TemplateEngine;
use anyhow::{Result, Context};
use async_trait::async_trait;
use serde_json::Value;

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
