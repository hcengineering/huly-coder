use std::fs;
// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use std::io;
use std::io::stdout;
use std::panic::set_hook;
use std::panic::take_hook;
use std::path::Path;

use crossterm::execute;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use providers::model_info::model_info;
use ratatui::prelude::CrosstermBackend;
use ratatui::DefaultTerminal;
use ratatui::Terminal;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

use self::config::Config;
use crate::agent::AgentControlEvent;
use crate::agent::AgentOutputEvent;
use clap::Parser;

mod agent;
mod config;
pub mod providers;
pub mod templates;
pub mod tools;
mod tui;

const HISTORY_PATH: &str = "history.json";
const CONFIG_STATE_FILE_PATH: &str = "config_state.yaml";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Skip loading previous session from history.json file
    #[arg(short, long)]
    skip_load_messages: bool,
    /// Path to data directory
    #[arg(short, long, default_value = "data")]
    data: String,
    /// Path to config file
    #[arg(short, long, default_value = "huly-coder-local.yaml")]
    config: String,
}

fn init_logger(data_dir: &str) {
    let log_dir = Path::new(data_dir).join("logs");
    let writer = tracing_appender::rolling::daily(log_dir, "huly-coder.log");
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(writer)
                .with_target(true)
                .with_filter(
                    tracing_subscriber::filter::Targets::new()
                        .with_target("ignore", tracing::Level::WARN)
                        .with_target("globset", tracing::Level::WARN)
                        .with_target("hyper_util::client::legacy", tracing::Level::INFO)
                        .with_target("html5ever", tracing::Level::WARN)
                        .with_target("tungstenite::protocol", tracing::Level::WARN)
                        .with_target("headless_chrome", tracing::Level::WARN)
                        .with_target("mio", tracing::Level::WARN)
                        .with_target("ort", tracing::Level::WARN)
                        .with_target("tokenizers", tracing::Level::WARN)
                        .with_target("process_wrap", tracing::Level::INFO)
                        .with_target("mcp_core::transport::client", tracing::Level::INFO)
                        .with_default(tracing::Level::DEBUG),
                ),
        )
        .init()
}

fn init_panic_hook() {
    let original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        // intentionally ignore errors here since we're already in a panic
        let backtrace = std::backtrace::Backtrace::capture();
        tracing::error!("{}, {:#?}", panic_info, backtrace);
        let _ = restore_tui();
        original_hook(panic_info);
        std::process::exit(1);
    }));
}

fn init_tui() -> io::Result<DefaultTerminal> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    // execute!(stdout(), crossterm::event::EnableMouseCapture)?;
    Terminal::new(CrosstermBackend::new(stdout()))
}

pub fn restore_tui() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    init_panic_hook();
    let args = Args::parse();

    init_logger(&args.data);

    tracing::info!("Start");
    let config = match Config::new(&args.config) {
        Ok(config) => config,
        Err(e) => {
            ratatui::restore();
            println!("Error: Failed to load config");
            return Err(e);
        }
    };
    let data_dir = Path::new(&args.data);
    if !data_dir.exists() {
        fs::create_dir_all(data_dir)?;
    }
    let history_path = data_dir.join(HISTORY_PATH);
    // start agent
    let (output_sender, output_receiver) =
        tokio::sync::mpsc::unbounded_channel::<AgentOutputEvent>();
    let (control_sender, control_receiver) =
        tokio::sync::mpsc::unbounded_channel::<AgentControlEvent>();
    let history = if !args.skip_load_messages && history_path.exists() {
        serde_json::from_str(&std::fs::read_to_string(history_path).unwrap()).unwrap()
    } else {
        Vec::new()
    };

    let model_info = model_info(&args.data, &config).await?;
    tracing::info!("Model info: {:?}", model_info);

    let mut agent = agent::Agent::new(&args.data, config.clone(), output_sender);
    let memory_index = agent.init_memory_index().await;

    let messages = history.clone();
    let agent_handler = tokio::spawn(async move {
        agent
            .run(&args.data, control_receiver, messages, memory_index)
            .await;
    });

    let terminal = init_tui().unwrap();
    let result = tui::App::new(config, model_info, control_sender, output_receiver, history)
        .run(terminal)
        .await;
    let _ = agent_handler.await;
    ratatui::restore();
    result
}
