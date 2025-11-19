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

    /// Resolve an expression like "global.api_url" or "nodes.fetch_data.output"
    fn resolve_expression(&self, expr: &str) -> Result<Value> {
        let parts: Vec<&str> = expr.split('.').collect();

        match parts.get(0) {
            Some(&"global") => {
                if parts.len() < 2 {
                    anyhow::bail!("Invalid global reference: {}", expr);
                }
                let key = parts[1];
                self.global
                    .get(key)
                    .with_context(|| format!("Global variable '{}' not found", key))
            }
            Some(&"nodes") => {
                if parts.len() < 3 {
                    anyhow::bail!("Invalid node reference: {}", expr);
                }
                let node_id = parts[1];
                let field = parts[2];

                match field {
                    "output" => self
                        .nodes
                        .get_output_value(node_id)
                        .with_context(|| format!("Node '{}' output not found", node_id)),
                    _ => anyhow::bail!("Unknown node field: {}", field),
                }
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
