use axum::{
    extract::{Json, Path, State},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::{Result, Context};
use serde_json::Value;
use uuid::Uuid;

use crate::schema::{Workflow, Node};
use crate::memory::NodeOutput;
use crate::worker::{ExecuteRequest, ExecuteResponse};

#[derive(Clone)]
struct CoordinatorState {
    inner: Arc<RwLock<CoordinatorInner>>,
}

struct CoordinatorInner {
    workers: Vec<WorkerInfo>,
    jobs: HashMap<String, JobState>,
    next_worker_index: usize,
}

#[derive(Clone)]
struct WorkerInfo {
    url: String,
    id: String,
}

struct JobState {
    workflow: Workflow,
    status: String,
    completed_nodes: HashSet<String>,
    node_outputs: HashMap<String, NodeOutput>,
    pending_nodes: VecDeque<String>,
    total_nodes: usize,
}

#[derive(Deserialize)]
pub struct SubmitRequest {
    pub workflow: Workflow,
}

#[derive(Serialize)]
pub struct SubmitResponse {
    pub job_id: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub job_id: String,
    pub status: String,
    pub progress: f64,
    pub completed: usize,
    pub total: usize,
    pub results: Option<HashMap<String, NodeOutput>>,
}

#[derive(Deserialize)]
pub struct RegisterWorkerRequest {
    pub worker_url: String,
    pub worker_id: String,
}

#[derive(Serialize)]
pub struct RegisterWorkerResponse {
    pub message: String,
    pub worker_count: usize,
}

pub async fn run_coordinator(port: u16) -> Result<()> {
    let state = CoordinatorState {
        inner: Arc::new(RwLock::new(CoordinatorInner {
            workers: Vec::new(),
            jobs: HashMap::new(),
            next_worker_index: 0,
        })),
    };

    let app = Router::new()
        .route("/submit", post(handle_submit))
        .route("/status/{job_id}", get(handle_status))
        .route("/register-worker", post(handle_register_worker))
        .route("/workers", get(handle_list_workers))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    log::info!("🎯 Coordinator starting on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn handle_submit(
    State(state): State<CoordinatorState>,
    Json(req): Json<SubmitRequest>,
) -> Json<SubmitResponse> {
    let job_id = Uuid::new_v4().to_string();
    
    log::info!("📥 Received workflow submission: {}", req.workflow.name);
    log::info!("   Job ID: {}", job_id);
    log::info!("   Total nodes: {}", req.workflow.nodes.len());

    // Initialize job state
    let total_nodes = req.workflow.nodes.len();
    let job_state = JobState {
        workflow: req.workflow.clone(),
        status: "pending".to_string(),
        completed_nodes: HashSet::new(),
        node_outputs: HashMap::new(),
        pending_nodes: VecDeque::new(),
        total_nodes,
    };

    {
        let mut inner = state.inner.write().await;
        inner.jobs.insert(job_id.clone(), job_state);
    }

    // Start execution in background
    let state_clone = state.clone();
    let job_id_clone = job_id.clone();
    tokio::spawn(async move {
        if let Err(e) = execute_workflow(state_clone, job_id_clone).await {
            log::error!("Workflow execution failed: {}", e);
        }
    });

    Json(SubmitResponse {
        job_id,
        message: format!("Workflow submitted with {} nodes", total_nodes),
    })
}

async fn execute_workflow(state: CoordinatorState, job_id: String) -> Result<()> {
    log::info!("🚀 Starting execution for job {}", job_id);

    // Update status to running
    {
        let mut inner = state.inner.write().await;
        if let Some(job) = inner.jobs.get_mut(&job_id) {
            job.status = "running".to_string();
        }
    }

    // Build dependency graph
    let (workflow, dependencies) = {
        let inner = state.inner.read().await;
        let job = inner.jobs.get(&job_id).context("Job not found")?;
        let mut deps = HashMap::new();
        for node in &job.workflow.nodes {
            deps.insert(node.id.clone(), node.needs.clone().unwrap_or_default());
        }
        (job.workflow.clone(), deps)
    };

    // Find initial ready nodes (no dependencies)
    let mut ready: VecDeque<String> = workflow
        .nodes
        .iter()
        .filter(|n| dependencies.get(&n.id).map(|d| d.is_empty()).unwrap_or(true))
        .map(|n| n.id.clone())
        .collect();

    log::info!("   Initial ready nodes: {}", ready.len());

    let mut in_flight: HashSet<String> = HashSet::new();

    // Execute until all nodes complete
    loop {
        // Schedule ready nodes
        while let Some(node_id) = ready.pop_front() {
            if in_flight.contains(&node_id) {
                continue;
            }

            let state_clone = state.clone();
            let job_id_clone = job_id.clone();
            let node_id_clone = node_id.clone();

            in_flight.insert(node_id.clone());

            tokio::spawn(async move {
                if let Err(e) = execute_node_distributed(
                    state_clone.clone(),
                    job_id_clone.clone(),
                    node_id_clone.clone(),
                )
                .await
                {
                    log::error!("Node {} execution failed: {}", node_id_clone, e);
                }
            });
        }

        // Wait a bit for nodes to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Check for newly ready nodes
        let (completed_count, total_count, newly_ready) = {
            let inner = state.inner.read().await;
            let job = inner.jobs.get(&job_id).context("Job  not found")?;
            
            let completed = job.completed_nodes.len();
            let total = job.total_nodes;

            // Find nodes that are now ready
            let mut newly_ready_nodes = Vec::new();
            for node in &workflow.nodes {
                if job.completed_nodes.contains(&node.id) || in_flight.contains(&node.id) {
                    continue;
                }

                let deps = dependencies.get(&node.id).cloned().unwrap_or_default();
                if deps.iter().all(|d| job.completed_nodes.contains(d)) {
                    newly_ready_nodes.push(node.id.clone());
                }
            }

            (completed, total, newly_ready_nodes)
        };

        // Add newly ready nodes to queue
        for node_id in newly_ready {
            ready.push_back(node_id.clone());
            in_flight.remove(&node_id);
        }

        // Check if done
        if completed_count == total_count {
            log::info!("✅ Workflow {} completed!", job_id);
            let mut inner = state.inner.write().await;
            if let Some(job) = inner.jobs.get_mut(&job_id) {
                job.status = "completed".to_string();
            }
            break;
        }

        // Safety: if nothing is in flight and nothing is ready, we're stuck
        if in_flight.is_empty() && ready.is_empty() && completed_count < total_count {
            log::error!("❌ Workflow {} is stuck! Completed: {}/{}", job_id, completed_count, total_count);
            let mut inner = state.inner.write().await;
            if let Some(job) = inner.jobs.get_mut(&job_id) {
                job.status = "failed".to_string();
            }
            break;
        }
    }

    Ok(())
}

async fn execute_node_distributed(
    state: CoordinatorState,
    job_id: String,
    node_id: String,
) -> Result<()> {
    log::info!("   [{}] Scheduling node...", node_id);

    // Get node and current state
    let (node, global_memory, node_outputs, worker) = {
        // First scope: read data
        let (node, global_map, node_outputs_map, worker_idx) = {
            let inner = state.inner.read().await;
            
            if inner.workers.is_empty() {
                return Err(anyhow::anyhow!("No workers available"));
            }

            let job = inner.jobs.get(&job_id).context("Job not found")?;
            
            let node = job
                .workflow
                .nodes
                .iter()
                .find(|n| n.id == node_id)
                .context("Node not found")?
                .clone();

            // Select worker index
            let worker_idx = inner.next_worker_index % inner.workers.len();

            // Prepare memory
            let global_map: HashMap<String, Value> = job.workflow.global.clone();
            let node_outputs_map = job.node_outputs.clone();

            (node, global_map, node_outputs_map, worker_idx)
        };

        // Second scope: update worker index and get worker
        let worker = {
            let mut inner = state.inner.write().await;
            inner.next_worker_index += 1;
            inner.workers[worker_idx].clone()
        };

        (node, global_map, node_outputs_map, worker)
    };

    log::info!("   [{}] Executing on worker: {}", node_id, worker.id);

    // Send to worker
    let client = reqwest::Client::new();
    let execute_req = ExecuteRequest {
        node: node.clone(),
        global_memory,
        node_outputs: node_outputs,
    };

    let response: ExecuteResponse = client
        .post(format!("{}/execute", worker.url))
        .json(&execute_req)
        .send()
        .await?
        .json()
        .await?;

    // Update job state
    {
        let mut inner = state.inner.write().await;
        let job = inner.jobs.get_mut(&job_id).context("Job not found")?;

        if response.status == "success" {
            if let Some(output) = response.output {
                job.node_outputs.insert(node_id.clone(), output);
                job.completed_nodes.insert(node_id.clone());
                log::info!("   [{}] ✓ Completed ({}/{})", node_id, job.completed_nodes.len(), job.total_nodes);
            }
        } else {
            log::error!("   [{}] ✗ Failed: {:?}", node_id, response.error);
        }
    }

    Ok(())
}

async fn handle_status(
    State(state): State<CoordinatorState>,
    Path(job_id): Path<String>,
) -> Json<StatusResponse> {
    let inner = state.inner.read().await;

    if let Some(job) = inner.jobs.get(&job_id) {
        let progress = job.completed_nodes.len() as f64 / job.total_nodes as f64;
        let results = if job.status == "completed" {
            Some(job.node_outputs.clone())
        } else {
            None
        };

        Json(StatusResponse {
            job_id,
            status: job.status.clone(),
            progress,
            completed: job.completed_nodes.len(),
            total: job.total_nodes,
            results,
        })
    } else {
        Json(StatusResponse {
            job_id,
            status: "not_found".to_string(),
            progress: 0.0,
            completed: 0,
            total: 0,
            results: None,
        })
    }
}

async fn handle_register_worker(
    State(state): State<CoordinatorState>,
    Json(req): Json<RegisterWorkerRequest>,
) -> Json<RegisterWorkerResponse> {
    let mut inner = state.inner.write().await;

    inner.workers.push(WorkerInfo {
        url: req.worker_url.clone(),
        id: req.worker_id.clone(),
    });

    let count = inner.workers.len();
    log::info!("✨ Worker registered: {} ({})", req.worker_id, req.worker_url);
    log::info!("   Total workers: {}", count);

    Json(RegisterWorkerResponse {
        message: format!("Worker {} registered successfully", req.worker_id),
        worker_count: count,
    })
}

async fn handle_list_workers(State(state): State<CoordinatorState>) -> Json<Value> {
    let inner = state.inner.read().await;
    let workers: Vec<_> = inner
        .workers
        .iter()
        .map(|w| serde_json::json!({"id": w.id, "url": w.url}))
        .collect();

    Json(serde_json::json!({
        "workers": workers,
        "count": workers.len()
    }))
}
