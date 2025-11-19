mod schema;
mod memory;
mod template;
mod nodes;
mod engine;

mod server;

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
