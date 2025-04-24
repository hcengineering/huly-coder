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

use self::config::Config;
use crate::agent::AgentControlEvent;
use crate::agent::AgentOutputEvent;

mod agent;
mod config;
pub mod providers;
pub mod templates;
pub mod tools;
mod tui;

fn init_logger() {
    let writer = tracing_appender::rolling::daily("logs", "huly-coder.log");
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(writer)
        .with_target(true)
        .init();
}

fn init_panic_hook() {
    let original_hook = take_hook();
    set_hook(Box::new(move |panic_info| {
        // intentionally ignore errors here since we're already in a panic
        tracing::error!("{}", panic_info);
        let _ = restore_tui();
        original_hook(panic_info);
        panic!();
    }));
}

fn init_tui() -> io::Result<DefaultTerminal> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
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
    dotenv::dotenv().ok();
    init_panic_hook();
    init_logger();
    tracing::info!("Start");
    let config = Config::load()?;
    // start agent
    let (output_sender, output_receiver) =
        tokio::sync::mpsc::unbounded_channel::<AgentOutputEvent>();
    let (control_sender, control_receiver) =
        tokio::sync::mpsc::unbounded_channel::<AgentControlEvent>();
    let mut agent = agent::Agent::new(config.clone(), control_receiver, output_sender);
    tokio::spawn(async move {
        agent.run().await;
    });

    let terminal = init_tui().unwrap();
    let result = tui::App::new(config, control_sender, output_receiver)
        .run(terminal)
        .await;
    ratatui::restore();
    result
}
