use anyhow::Result;
use async_trait::async_trait;
use crate::memory::{NodeOutput, GlobalMemory, NodeMemory};
use crate::schema::Node;

mod shell;
mod http;
mod script;
mod llm;
mod transform;
mod file;
mod control;
mod loop_node;
mod input;

pub use shell::ShellExecutor;
pub use http::HttpExecutor;
pub use script::ScriptExecutor;
pub use llm::LlmExecutor;
pub use transform::TransformExecutor;
pub use file::FileExecutor;
pub use control::{DelayExecutor, SwitchExecutor};
pub use loop_node::LoopExecutor;
pub use input::InputExecutor;

#[async_trait]
pub trait NodeExecutor: Send + Sync {
    async fn execute(
        &self,
        node: &Node,
        global: &GlobalMemory,
        nodes: &NodeMemory,
    ) -> Result<NodeOutput>;
}

pub fn get_executor(node_type: &str) -> Result<Box<dyn NodeExecutor>> {
    match node_type {
        "shell" => Ok(Box::new(ShellExecutor)),
        "http" => Ok(Box::new(HttpExecutor)),
        "delay" => Ok(Box::new(DelayExecutor)),
        "switch" => Ok(Box::new(SwitchExecutor)),
        "script" => Ok(Box::new(ScriptExecutor)),
        "llm" => Ok(Box::new(LlmExecutor)),
        "transform" => Ok(Box::new(TransformExecutor)),
        "file" => Ok(Box::new(FileExecutor)),
        "input" => Ok(Box::new(InputExecutor)),
        "loop" => Ok(Box::new(LoopExecutor)),
        _ => anyhow::bail!("Unknown node type: {}", node_type),
    }
}
