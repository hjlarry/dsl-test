use crate::executor::get_executor;
use crate::memory::{GlobalMemory, NodeMemory};
use crate::schema::Workflow;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct Engine {
    workflow: Workflow,
    global_memory: GlobalMemory,
    node_memory: NodeMemory,
}

impl Engine {
    pub fn new(workflow: Workflow) -> Self {
        let global_memory = GlobalMemory::new();
        
        // Initialize global memory with workflow globals
        for (key, value) in workflow.global.iter() {
            global_memory.set(key.clone(), value.clone());
        }

        Self {
            workflow,
            global_memory,
            node_memory: NodeMemory::new(),
        }
    }

    /// Execute the workflow with automatic parallelization based on dependencies
    pub async fn execute(&self) -> Result<()> {
        log::info!("Starting workflow execution: {}", self.workflow.name);

        // Build dependency graph
        let mut dependencies: HashMap<String, HashSet<String>> = HashMap::new();
        let mut node_map = HashMap::new();

        for node in &self.workflow.nodes {
            node_map.insert(node.id.clone(), node.clone());
            dependencies.insert(node.id.clone(), node.needs.iter().cloned().collect());
        }

        // Track completed nodes
        let mut completed: HashSet<String> = HashSet::new();
        let mut in_progress: HashSet<String> = HashSet::new();

        // Limit concurrent execution
        let max_concurrency = 10;
        let semaphore = Arc::new(Semaphore::new(max_concurrency));

        loop {
            // Find nodes that are ready to execute (all dependencies met)
            let mut ready: Vec<String> = Vec::new();
            
            for (node_id, deps) in &dependencies {
                if !completed.contains(node_id) 
                    && !in_progress.contains(node_id) 
                    && deps.iter().all(|dep| completed.contains(dep)) 
                {
                    ready.push(node_id.clone());
                }
            }

            if ready.is_empty() {
                // Check if all nodes are done
                if completed.len() == self.workflow.nodes.len() {
                    break; // Workflow complete
                } else if in_progress.is_empty() {
                    // No ready nodes and nothing in progress = deadlock or missing dependency
                    anyhow::bail!("Workflow is stuck. Possible circular dependency or missing nodes.");
                }
                
                // Wait a bit for in-progress tasks
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                continue;
            }

            // Execute ready nodes in parallel
            let mut handles = Vec::new();

            for node_id in ready {
                let node = node_map.get(&node_id).unwrap().clone();
                let global = self.global_memory.clone();
                let nodes = self.node_memory.clone();
                let permit = semaphore.clone().acquire_owned().await.unwrap();

                in_progress.insert(node_id.clone());

                let handle = tokio::spawn(async move {
                    let _permit = permit; // Hold permit until task completes
                    
                    log::info!("Executing node: {} ({})", node.name, node.id);
                    
                    let executor = get_executor(&node.node_type)?;
                    let result = executor.execute(&node, &global, &nodes).await;
                    
                    match result {
                        Ok(output) => {
                            log::info!("Node {} completed with status: {}", node.id, output.status);
                            nodes.set(node.id.clone(), output);
                            Ok(node.id)
                        }
                        Err(e) => {
                            log::error!("Node {} failed: {}", node.id, e);
                            Err(e)
                        }
                    }
                });

                handles.push(handle);
            }

            // Wait for all spawned tasks to complete
            for handle in handles {
                match handle.await {
                    Ok(Ok(node_id)) => {
                        completed.insert(node_id.clone());
                        in_progress.remove(&node_id);
                    }
                    Ok(Err(e)) => {
                        return Err(e).context("Node execution failed");
                    }
                    Err(e) => {
                        return Err(e).context("Task join failed");
                    }
                }
            }
        }

        log::info!("Workflow execution completed successfully");
        Ok(())
    }

    pub fn get_node_memory(&self) -> &NodeMemory {
        &self.node_memory
    }

    pub fn get_global_memory(&self) -> &GlobalMemory {
        &self.global_memory
    }
}
