// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use std::io;
use std::io::stdout;
use std::panic::set_hook;
use std::panic::take_hook;

use crossterm::execute;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
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

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Skip loading previous session from history.json file
    #[arg(short, long)]
    skip_load_messages: bool,
}

fn init_logger() {
    let writer = tracing_appender::rolling::daily("logs", "huly-coder.log");
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
                        .with_default(tracing::Level::TRACE),
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
    init_logger();
    let args = Args::parse();

    tracing::info!("Start");
    let config = Config::new()?;
    // start agent
    let (output_sender, output_receiver) =
        tokio::sync::mpsc::unbounded_channel::<AgentOutputEvent>();
    let (control_sender, control_receiver) =
        tokio::sync::mpsc::unbounded_channel::<AgentControlEvent>();
    let history = if !args.skip_load_messages && std::path::Path::new("history.json").exists() {
        serde_json::from_str(&std::fs::read_to_string("history.json").unwrap()).unwrap()
    } else {
        Vec::new()
    };

    let mut agent = agent::Agent::new(
        config.clone(),
        control_receiver,
        output_sender,
        history.clone(),
    );
    agent.init_memory_index().await;

    let agent_handler = tokio::spawn(async move {
        agent.run().await;
    });

    let terminal = init_tui().unwrap();
    let result = tui::App::new(config, control_sender, output_receiver, history)
        .run(terminal)
        .await;
    let _ = agent_handler.await;
    ratatui::restore();
    result
}
