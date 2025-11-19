use axum::{
    extract::Json,
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::collections::HashMap;
use serde_json::Value;
use anyhow::Context;
use crate::engine::Engine;
use crate::schema;

#[derive(Deserialize)]
pub struct ExecuteRequest {
    pub file: String,
    pub inputs: Option<HashMap<String, Value>>,
}

#[derive(Serialize)]
pub struct ExecuteResponse {
    pub status: String,
    pub outputs: HashMap<String, Value>,
    pub error: Option<String>,
}

pub async fn run_server(port: u16) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/execute", post(handle_execute));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    log::info!("🚀 Server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn handle_execute(
    Json(payload): Json<ExecuteRequest>,
) -> Json<ExecuteResponse> {
    log::info!("Received execution request for file: {}", payload.file);

    match execute_workflow(payload).await {
        Ok(outputs) => Json(ExecuteResponse {
            status: "success".to_string(),
            outputs,
            error: None,
        }),
        Err(e) => {
            log::error!("Execution failed: {}", e);
            Json(ExecuteResponse {
                status: "error".to_string(),
                outputs: HashMap::new(),
                error: Some(e.to_string()),
            })
        }
    }
}

async fn execute_workflow(req: ExecuteRequest) -> anyhow::Result<HashMap<String, Value>> {
    // Read workflow file
    let content = tokio::fs::read_to_string(&req.file)
        .await
        .with_context(|| format!("Could not read file `{}`", req.file))?;

    // Parse workflow
    let mut workflow: schema::Workflow = serde_yaml::from_str(&content)
        .context("Failed to parse YAML workflow")?;

    // Inject inputs
    if let Some(inputs) = req.inputs {
        for (k, v) in inputs {
            workflow.global.insert(k, v);
        }
    }

    // Execute
    let engine = Engine::new(workflow);
    engine.execute().await?;

    // Return outputs
    Ok(engine.get_node_memory().get_all_values())
}
