mod schema;
mod memory;
mod template;
mod nodes;
mod engine;

mod server;
mod worker;
mod coordinator;

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use anyhow::{Context, Result};
use std::fs;
use engine::Engine;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    /// Default pretty-printed format
    Pretty,
    /// JSON format
    Json,
    /// Markdown format
    Markdown,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    // Keep these as optional for backward compatibility (default run mode)
    /// Path to the workflow YAML file
    #[arg(short, long, value_name = "FILE")]
    file: Option<PathBuf>,

    /// Input parameters in key=value format
    #[arg(short, long, value_name = "KEY=VALUE")]
    input: Option<Vec<String>>,

    /// Output format
    #[arg(short = 'o', long, value_enum, default_value_t = OutputFormat::Pretty)]
    format: OutputFormat,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a workflow file (default)
    Run {
        /// Path to the workflow YAML file
        #[arg(short, long, value_name = "FILE")]
        file: PathBuf,

        /// Input parameters in key=value format
        #[arg(short, long, value_name = "KEY=VALUE")]
        input: Vec<String>,

        /// Output format
        #[arg(short = 'o', long, value_enum, default_value_t = OutputFormat::Pretty)]
        format: OutputFormat,
    },
    /// Start the webhook server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
    /// Start a distributed worker
    Worker {
        /// Worker ID
        #[arg(short, long)]
        id: String,

        /// Port to listen on
        #[arg(short, long, default_value = "3001")]
        port: u16,

        /// Coordinator URL to register with
        #[arg(short, long)]
        coordinator: Option<String>,
    },
    /// Start the distributed coordinator
    Coordinator {
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    /// Submit a workflow to the coordinator
    Submit {
        /// Path to the workflow YAML file
        #[arg(short, long, value_name = "FILE")]
        file: PathBuf,

        /// Coordinator URL
        #[arg(short, long, default_value = "http://localhost:8080")]
        coordinator: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if it exists
    dotenv::dotenv().ok();
    
    env_logger::init();
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Serve { port }) => {
            server::run_server(port).await?;
        }
        Some(Commands::Run { file, input, format }) => {
            run_workflow(file, input, format).await?;
        }
        Some(Commands::Coordinator { port }) => {
            coordinator::run_coordinator(port).await?;
        }
        Some(Commands::Worker { id, port, coordinator }) => {
            // Start worker
            let worker_url = format!("http://localhost:{}", port);
            let id_clone = id.clone();
            
            // Register with coordinator if specified
            if let Some(coord_url) = coordinator {
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    register_worker(coord_url, worker_url.clone(), id_clone).await.ok();
                });
            }
            
            worker::run_worker(id, port).await?;
        }
        Some(Commands::Submit { file, coordinator }) => {
            submit_workflow(file, coordinator).await?;
        }
        None => {
            // Default behavior: check if file arg is present
            if let Some(file) = cli.file {
                let input = cli.input.unwrap_or_default();
                run_workflow(file, input, cli.format).await?;
            } else {
                // Print help if no args
                use clap::CommandFactory;
                Cli::command().print_help()?;
            }
        }
    }
    
    Ok(())
}

async fn run_workflow(file: PathBuf, input: Vec<String>, format: OutputFormat) -> Result<()> {
    println!("🚀 Loading workflow from: {:?}", file);

    let content = fs::read_to_string(&file)
        .with_context(|| format!("Could not read file `{:?}`", file))?;

    let mut workflow: schema::Workflow = serde_yaml::from_str(&content)
        .context("Failed to parse YAML workflow")?;

    // Override/Add globals from CLI
    for inp in input {
        if let Some((key, value_str)) = inp.split_once('=') {
            let value = serde_json::from_str(value_str)
                .unwrap_or_else(|_| serde_json::Value::String(value_str.to_string()));
            
            workflow.global.insert(key.to_string(), value);
        } else {
            log::warn!("Invalid input format: {}", inp);
        }
    }

    match format {
        OutputFormat::Pretty | OutputFormat::Markdown => {
            println!("✅ Workflow parsed: {}", workflow.name);
            println!("📊 Global vars: {:?}", workflow.global);
            println!("🔢 Nodes count: {}", workflow.nodes.len());
            println!();
        },
        OutputFormat::Json => {
            // No detailed print for JSON output, as the final output will be JSON
        }
    }

    // Execute the workflow
    let engine = Engine::new(workflow);
    engine.execute().await?;

    match format {
        OutputFormat::Pretty => {
            println!();
            println!("✨ Workflow execution completed!");
            
            println!("\n📊 Final Execution Summary:");
            println!("----------------------------------------");
            
            println!("Global Memory:");
            let globals = engine.get_global_memory().get_all();
            if globals.is_empty() {
                println!("  (empty)");
            } else {
                for (k, v) in globals {
                    println!("  {}: {}", k, serde_json::to_string_pretty(&v).unwrap_or_default());
                }
            }
            
            println!("\nNode Outputs:");
            let outputs = engine.get_node_memory().get_all_values();
            if outputs.is_empty() {
                println!("  (empty)");
            } else {
                for (k, v) in outputs {
                    println!("  {}: {}", k, serde_json::to_string_pretty(&v).unwrap_or_default());
                }
            }
            println!("----------------------------------------");
        },
        OutputFormat::Json => {
            let mut result_json = serde_json::Map::new();
            let globals_map: serde_json::Map<String, serde_json::Value> = engine.get_global_memory().get_all().into_iter().collect();
            let outputs_map: serde_json::Map<String, serde_json::Value> = engine.get_node_memory().get_all_values().into_iter().collect();
            
            result_json.insert("global_memory".to_string(), serde_json::Value::Object(globals_map));
            result_json.insert("node_outputs".to_string(), serde_json::Value::Object(outputs_map));
            
            println!("{}", serde_json::to_string_pretty(&serde_json::Value::Object(result_json)).unwrap_or_default());
        },
        OutputFormat::Markdown => {
            println!();
            println!("✨ Workflow execution completed!");
            
            println!("\n# Final Execution Summary");
            
            println!("\n## Global Memory");
            let globals = engine.get_global_memory().get_all();
            if globals.is_empty() {
                println!("  *(empty)*");
            } else {
                for (k, v) in globals {
                    println!("### `{}`\n```json\n{}\n```", k, serde_json::to_string_pretty(&v).unwrap_or_default());
                }
            }
            
            println!("\n## Node Outputs");
            let outputs = engine.get_node_memory().get_all_values();
            if outputs.is_empty() {
                println!("  *(empty)*");
            } else {
                for (k, v) in outputs {
                    println!("### `{}`\n```json\n{}\n```", k, serde_json::to_string_pretty(&v).unwrap_or_default());
                }
            }
        }
    }
    
    Ok(())
}

async fn register_worker(coordinator_url: String, worker_url: String, worker_id: String) -> Result<()> {
    log::info!("📝 Registering worker {} with coordinator...", worker_id);
    
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/register-worker", coordinator_url))
        .json(&serde_json::json!({
            "worker_url": worker_url,
            "worker_id": worker_id
        }))
        .send()
        .await?;
    
    if resp.status().is_success() {
        log::info!("✅ Worker registered successfully");
    } else {
        log::error!("❌ Failed to register worker: {}", resp.status());
    }
    
    Ok(())
}

async fn submit_workflow(file: PathBuf, coordinator_url: String) -> Result<()> {
    println!("📤 Submitting workflow to coordinator...");
    
    let content = fs::read_to_string(&file)
        .with_context(|| format!("Failed to read file: {:?}", file))?;
    
    let workflow: schema::Workflow = serde_yaml::from_str(&content)?;
    
    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .post(format!("{}/submit", coordinator_url))
        .json(&serde_json::json!({
            "workflow": workflow
        }))
        .send()
        .await?
        .json()
        .await?;
    
    println!("✅ Workflow submitted!");
    println!("   Job ID: {}", resp["job_id"]);
    println!("   {}", resp["message"]);
    
    let job_id = resp["job_id"].as_str().unwrap();
    
    // Poll for status
    println!("\n📊 Monitoring execution...");
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        let status: serde_json::Value = client
            .get(format!("{}/status/{}", coordinator_url, job_id))
            .send()
            .await?
            .json()
            .await?;
        
        let state = status["status"].as_str().unwrap();
        let completed = status["completed"].as_u64().unwrap();
        let total = status["total"].as_u64().unwrap();
        let progress = status["progress"].as_f64().unwrap() * 100.0;
        
        println!("   Status: {} - {}/{} nodes ({:.1}%)", state, completed, total, progress);
        
        if state == "completed" {
            println!("\n✨ Workflow completed successfully!");
            if let Some(results) = status["results"].as_object() {
                println!("\n📊 Results:");
                for (k, v) in results {
                    println!("   {}: {}", k, serde_json::to_string_pretty(v)?);
                }
            }
            break;
        } else if state == "failed" {
            println!("\n❌ Workflow failed!");
            break;
        }
    }
    
    Ok(())
}
