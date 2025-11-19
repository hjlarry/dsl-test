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
        // Split by dot, but we need to handle array indexing like users[0]
        // This is a simple implementation. For full JSONPath support, we'd need a parser.
        // Here we convert "users[0].name" -> ["users", "0", "name"]
        let parts: Vec<String> = expr
            .replace('[', ".")
            .replace(']', "")
            .split('.')
            .map(|s| s.to_string())
            .collect();
        
        let parts_refs: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();

        match parts_refs.get(0) {
            Some(&"global") => {
                if parts_refs.len() < 2 {
                    anyhow::bail!("Invalid global reference: {}", expr);
                }
                let key = parts_refs[1];
                let value = self
                    .global
                    .get(key)
                    .with_context(|| format!("Global variable '{}' not found", key))?;
                
                self.traverse_path(&value, &parts_refs[2..])
            }
            Some(&"nodes") => {
                if parts_refs.len() < 3 {
                    anyhow::bail!("Invalid node reference: {}", expr);
                }
                let node_id = parts_refs[1];
                let field = parts_refs[2];

                match field {
                    "output" => {
                        let output = self
                            .nodes
                            .get_output_value(node_id)
                            .with_context(|| format!("Node '{}' output not found", node_id))?;
                        
                        self.traverse_path(&output, &parts_refs[3..])
                    }
                    _ => anyhow::bail!("Unknown node field: {}", field),
                }
            }
            Some(&"loop") => {
                let loop_ctx = self.global.get("loop")
                    .context("Loop context not found (are you inside a loop node?)")?;
                
                if parts_refs.len() < 2 {
                     return Ok(loop_ctx);
                }
                
                self.traverse_path(&loop_ctx, &parts_refs[1..])
            }
            _ => anyhow::bail!("Unknown expression prefix: {}", expr),
        }
    }

    fn traverse_path(&self, value: &Value, path: &[&str]) -> Result<Value> {
        let mut current = value.clone();
        for &key in path {
            if let Ok(index) = key.parse::<usize>() {
                if let Some(arr) = current.as_array() {
                    if let Some(v) = arr.get(index) {
                        current = v.clone();
                        continue;
                    }
                }
            }
            
            current = current.get(key)
                .cloned()
                .with_context(|| format!("Field '{}' not found", key))?;
        }
        Ok(current)
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
