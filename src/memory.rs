use dashmap::DashMap;
use serde_json::Value;
use std::sync::Arc;

/// Global memory shared across all nodes
#[derive(Clone, Debug)]
pub struct GlobalMemory {
    data: Arc<DashMap<String, Value>>,
}

impl GlobalMemory {
    pub fn new() -> Self {
        Self {
            data: Arc::new(DashMap::new()),
        }
    }

    pub fn set(&self, key: String, value: Value) {
        self.data.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        self.data.get(key).map(|v| v.clone())
    }

    pub fn get_all(&self) -> Vec<(String, Value)> {
        self.data
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }
}

/// Node output storage - stores results of each node execution
#[derive(Clone, Debug)]
pub struct NodeMemory {
    outputs: Arc<DashMap<String, NodeOutput>>,
}

#[derive(Clone, Debug)]
pub struct NodeOutput {
    pub status: String,
    pub output: Value,
}

impl NodeMemory {
    pub fn new() -> Self {
        Self {
            outputs: Arc::new(DashMap::new()),
        }
    }

    pub fn set(&self, node_id: String, output: NodeOutput) {
        self.outputs.insert(node_id, output);
    }

    pub fn get(&self, node_id: &str) -> Option<NodeOutput> {
        self.outputs.get(node_id).map(|v| v.clone())
    }

    pub fn get_output_value(&self, node_id: &str) -> Option<Value> {
        self.outputs.get(node_id).map(|v| v.output.clone())
    }
}
