mod schema;
mod memory;
mod template;
mod nodes;
mod engine;

use clap::Parser;
use std::path::PathBuf;
use anyhow::{Context, Result};
use std::fs;
use engine::Engine;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the workflow YAML file
    #[arg(short, long, value_name = "FILE")]
    file: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if it exists
    dotenv::dotenv().ok();
    
    env_logger::init();
    let cli = Cli::parse();

    println!("🚀 Loading workflow from: {:?}", cli.file);

    let content = fs::read_to_string(&cli.file)
        .with_context(|| format!("Could not read file `{:?}`", cli.file))?;

    let workflow: schema::Workflow = serde_yaml::from_str(&content)
        .context("Failed to parse YAML workflow")?;

    println!("✅ Workflow parsed: {}", workflow.name);
    println!("📊 Global vars: {:?}", workflow.global);
    println!("🔢 Nodes count: {}", workflow.nodes.len());
    println!();

    // Execute the workflow
    let engine = Engine::new(workflow);
    engine.execute().await?;

    println!();
    println!("✨ Workflow execution completed!");
    
    Ok(())
}
