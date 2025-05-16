// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use crate::AgentOutputEvent;
use color_eyre::eyre::OptionExt;
use futures::{FutureExt, StreamExt};
use ratatui::crossterm::event::Event as CrosstermEvent;
use std::time::Duration;
use tokio::sync::mpsc;

/// The frequency at which tick events are emitted.
const TICK_FPS: f64 = 2.0;

#[derive(Clone, Debug)]
pub enum UiEvent {
    /// Fixed rate tick event.
    Tick,
    /// Crossterm events.
    Crossterm(CrosstermEvent),
    /// Application events.
    App(AppEvent),
}

#[derive(Clone, Debug)]
pub enum AppEvent {
    Quit,
    Agent(AgentOutputEvent),
}

#[derive(Debug)]
pub struct UiEventMultiplexer {
    sender: mpsc::UnboundedSender<UiEvent>,
    receiver: mpsc::UnboundedReceiver<UiEvent>,
}

impl UiEventMultiplexer {
    pub fn new(agent_receiver: mpsc::UnboundedReceiver<AgentOutputEvent>) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let mut actor = UiEventTask::new(sender.clone(), agent_receiver);
        tokio::spawn(async move { actor.run().await });
        Self { sender, receiver }
    }

    pub async fn next(&mut self) -> color_eyre::Result<UiEvent> {
        self.receiver
            .recv()
            .await
            .ok_or_eyre("Failed to receive event")
    }

    pub fn send(&mut self, app_event: AppEvent) {
        let _ = self.sender.send(UiEvent::App(app_event));
    }
}

struct UiEventTask {
    sender: mpsc::UnboundedSender<UiEvent>,
    agent_receiver: mpsc::UnboundedReceiver<AgentOutputEvent>,
}

impl UiEventTask {
    pub fn new(
        sender: mpsc::UnboundedSender<UiEvent>,
        agent_receiver: mpsc::UnboundedReceiver<AgentOutputEvent>,
    ) -> Self {
        Self {
            sender,
            agent_receiver,
        }
    }

    async fn run(&mut self) -> color_eyre::Result<()> {
        let tick_rate = Duration::from_secs_f64(1.0 / TICK_FPS);
        let mut reader = crossterm::event::EventStream::new();
        let mut tick = tokio::time::interval(tick_rate);
        loop {
            let tick_delay = tick.tick();
            let crossterm_event = reader.next().fuse();
            tokio::select! {
              _ = self.sender.closed() => {
                break;
              }
              _ = tick_delay => {
                let _ = self.sender.send(UiEvent::Tick);
              }
              Some(Ok(evt)) = crossterm_event => {
                let _ = self.sender.send(UiEvent::Crossterm(evt));
              }
              Some(evt) = self.agent_receiver.recv() => {
                let _ = self.sender.send(UiEvent::App(AppEvent::Agent(evt)));
              }
            };
        }
        Ok(())
    }
}
