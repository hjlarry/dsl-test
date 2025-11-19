use axum::{
    extract::{Json, State},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use anyhow::Result;
use serde_json::Value;

use crate::schema::Node;
use crate::memory::{GlobalMemory, NodeMemory, NodeOutput};
use crate::nodes::get_executor;

#[derive(Clone)]
struct WorkerState {
    id: String,
}

#[derive(Deserialize, Serialize)]
pub struct ExecuteRequest {
    pub node: Node,
    pub global_memory: HashMap<String, Value>,
    pub node_outputs: HashMap<String, NodeOutput>,
}

#[derive(Serialize, Deserialize)]
pub struct ExecuteResponse {
    pub status: String,
    pub output: Option<NodeOutput>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub worker_id: String,
}

pub async fn run_worker(worker_id: String, port: u16) -> Result<()> {
    let state = WorkerState {
        id: worker_id.clone(),
    };

    let app = Router::new()
        .route("/execute", post(handle_execute))
        .route("/health", get(handle_health))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    log::info!("🔧 Worker {} starting on http://{}", worker_id, addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn handle_execute(
    State(state): State<WorkerState>,
    Json(req): Json<ExecuteRequest>,
) -> Json<ExecuteResponse> {
    log::info!("[Worker {}] Executing node: {}", state.id, req.node.id);

    // Reconstruct memory from request
    let global = GlobalMemory::new();
    for (k, v) in req.global_memory {
        global.set(k, v);
    }

    let nodes = NodeMemory::new();
    for (k, v) in req.node_outputs {
        nodes.set(k, v);
    }

    // Execute the node
    match execute_node(&req.node, &global, &nodes).await {
        Ok(output) => {
            log::info!("[Worker {}] Node {} completed successfully", state.id, req.node.id);
            Json(ExecuteResponse {
                status: "success".to_string(),
                output: Some(output),
                error: None,
            })
        }
        Err(e) => {
            log::error!("[Worker {}] Node {} failed: {}", state.id, req.node.id, e);
            Json(ExecuteResponse {
                status: "failed".to_string(),
                output: None,
                error: Some(e.to_string()),
            })
        }
    }
}

async fn handle_health(State(state): State<WorkerState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        worker_id: state.id,
    })
}

async fn execute_node(
    node: &Node,
    global: &GlobalMemory,
    nodes: &NodeMemory,
) -> Result<NodeOutput> {
    let executor = get_executor(&node.node_type)?;
    executor.execute(node, global, nodes).await
}
