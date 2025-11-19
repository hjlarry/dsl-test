use crate::memory::{GlobalMemory, NodeMemory};
use anyhow::{Result, Context};
use regex::Regex;
use serde_json::Value;

pub struct TemplateEngine {
    global: GlobalMemory,
    nodes: NodeMemory,
}

impl TemplateEngine {
    pub fn new(global: GlobalMemory, nodes: NodeMemory) -> Self {
        Self { global, nodes }
    }

    /// Replace variables in format {{ global.key }} or {{ nodes.id.output }}
    pub fn render(&self, template: &str) -> Result<String> {
        let re = Regex::new(r"\{\{\s*([^}]+)\s*\}\}").unwrap();
        let mut result = template.to_string();

        for cap in re.captures_iter(template) {
            let full_match = &cap[0];
            let expr = cap[1].trim();

            let value = self.resolve_expression(expr)?;
            let replacement = self.value_to_string(&value);

            result = result.replace(full_match, &replacement);
        }

        Ok(result)
    }

    /// Resolve an expression like "global.api_url" or "nodes.fetch_data.output.stdout"
    fn resolve_expression(&self, expr: &str) -> Result<Value> {
        let parts: Vec<&str> = expr.split('.').collect();

        match parts.get(0) {
            Some(&"global") => {
                if parts.len() < 2 {
                    anyhow::bail!("Invalid global reference: {}", expr);
                }
                let key = parts[1];
                let value = self
                    .global
                    .get(key)
                    .with_context(|| format!("Global variable '{}' not found", key))?;
                
                // Support nested access like global.obj.field
                if parts.len() > 2 {
                    let mut current = value;
                    for &field_name in &parts[2..] {
                        current = current.get(field_name)
                            .cloned()
                            .with_context(|| format!("Field '{}' not found in global variable '{}'", field_name, key))?;
                    }
                    Ok(current)
                } else {
                    Ok(value)
                }
            }
            Some(&"nodes") => {
                if parts.len() < 3 {
                    anyhow::bail!("Invalid node reference: {}", expr);
                }
                let node_id = parts[1];
                let field = parts[2];

                match field {
                    "output" => {
                        let output = self
                            .nodes
                            .get_output_value(node_id)
                            .with_context(|| format!("Node '{}' output not found", node_id))?;
                        
                        // Support nested access like nodes.id.output.stdout
                        if parts.len() > 3 {
                            let mut current = output;
                            for &field_name in &parts[3..] {
                                current = current.get(field_name)
                                    .cloned()
                                    .with_context(|| format!("Field '{}' not found in output", field_name))?;
                            }
                            Ok(current)
                        } else {
                            Ok(output)
                        }
                    }
                    _ => anyhow::bail!("Unknown node field: {}", field),
                }
            }
            Some(&"loop") => {
                // Look for "loop" object in global memory
                let loop_ctx = self.global.get("loop")
                    .context("Loop context not found (are you inside a loop node?)")?;
                
                // parts[0] is "loop"
                if parts.len() < 2 {
                     return Ok(loop_ctx);
                }
                
                let mut current = loop_ctx;
                for &field_name in &parts[1..] {
                    current = current.get(field_name)
                        .cloned()
                        .with_context(|| format!("Field '{}' not found in loop context", field_name))?;
                }
                Ok(current)
            }
            _ => anyhow::bail!("Unknown expression prefix: {}", expr),
        }
    }

    fn value_to_string(&self, value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            _ => value.to_string(),
        }
    }
}
