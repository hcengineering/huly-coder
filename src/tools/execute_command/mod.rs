// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.

use std::collections::HashMap;
use std::process::ExitStatus;

use anyhow::Result;
use process_wrap::tokio::{TokioChildWrapper, TokioCommandWrap};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{ChildStderr, ChildStdout};
use tokio::sync::{mpsc, oneshot};

use crate::agent::event::AgentCommandStatus;

pub mod tools;

#[cfg(unix)]
const SHELL: &str = "bash";
#[cfg(windows)]
const SHELL: &str = "cmd";

#[derive(Default)]
pub struct ProcessRegistry {
    counter: usize,
    processes: HashMap<usize, ProcessData>,
}

struct ProcessData {
    command: String,
    output: String,
    exit_status: Option<ExitStatus>,
    receiver: mpsc::UnboundedReceiver<ProcessOutput>,
    terminate_sender: Option<oneshot::Sender<()>>,
}

enum ProcessOutput {
    Exited(ExitStatus),
    Output(String),
    Error(String),
}

struct ProcessRuntime {
    _process: Box<dyn TokioChildWrapper>,
    stdout: ChildStdout,
    stderr: ChildStderr,
    sender: mpsc::UnboundedSender<ProcessOutput>,
    terminate_signal: oneshot::Receiver<()>,
}

impl ProcessRuntime {
    pub async fn run(mut self) {
        use tokio::pin;

        let stdout = Self::handle_stdout(self.stdout, self.sender.clone());
        let stderr = Self::handle_stderr(self.stderr, self.sender.clone());
        let status = Box::into_pin(self._process.wait());
        pin!(stdout);
        pin!(stderr);
        let mut exit_status = ExitStatus::default();
        tokio::select! {
            result = &mut stdout => {
                tracing::trace!("Stdout handler completed: {:?}", result);
            }
            result = &mut stderr => {
                tracing::trace!("Stderr handler completed: {:?}", result);
            }
            // capture the status so we don't need to wait for a timeout
            result = status => {
                if let Ok(result) = result {
                    exit_status = result;
                }
                tracing::trace!("Process exited with status: {:?}", result);
            }
            _ = self.terminate_signal => {
                tracing::debug!("Receive terminal_signal");
                if self._process.start_kill().is_ok() {
                    if let Ok(status) = Box::into_pin(self._process.wait()).await {
                        exit_status = status;
                    }
                }
            }
        }
        self.sender.send(ProcessOutput::Exited(exit_status)).ok();
    }

    async fn handle_stdout(stdout: ChildStdout, sender: mpsc::UnboundedSender<ProcessOutput>) {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    break;
                } // EOF
                Ok(_) => {
                    if sender.send(ProcessOutput::Output(line.clone())).is_err() {
                        break;
                    }
                    line.clear();
                }
                Err(_) => {
                    break;
                }
            }
        }
    }

    async fn handle_stderr(stderr: ChildStderr, sender: mpsc::UnboundedSender<ProcessOutput>) {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        loop {
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    break;
                } // EOF
                Ok(_) => {
                    if sender.send(ProcessOutput::Error(line.clone())).is_err() {
                        break;
                    }
                    line.clear();
                }
                Err(_) => {
                    break;
                }
            }
        }
    }
}

impl ProcessRegistry {
    async fn spawn_process(
        &self,
        command: &str,
        cwd: &str,
    ) -> Result<(Box<dyn TokioChildWrapper>, ChildStdout, ChildStderr)> {
        let mut child = TokioCommandWrap::with_new(SHELL, |cmd| {
            cmd.current_dir(cwd)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            #[cfg(unix)]
            cmd.arg("-c");

            #[cfg(windows)]
            cmd.arg("/C");

            cmd.arg(command);
        });
        child.wrap(process_wrap::tokio::KillOnDrop);

        #[cfg(unix)]
        child.wrap(process_wrap::tokio::ProcessGroup::leader());
        #[cfg(windows)]
        child.wrap(process_wrap::tokio::JobObject);

        let mut process = child.spawn()?;

        let stdout = process
            .stdout()
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get stdout"))?;

        let stderr = process
            .stderr()
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get stdout"))?;

        Ok((process, stdout, stderr))
    }

    pub async fn execute_command(&mut self, command: &str, cwd: &str) -> Result<usize> {
        self.counter = self.counter.saturating_add(1);
        let (process, stdout, stderr) = self.spawn_process(command, cwd).await?;
        let (tx, rx) = mpsc::unbounded_channel();
        let (t_tx, t_rx) = tokio::sync::oneshot::channel();

        let runtime = ProcessRuntime {
            _process: process,
            stdout,
            stderr,
            sender: tx,
            terminate_signal: t_rx,
        };

        tokio::spawn(runtime.run());

        self.processes.insert(
            self.counter,
            ProcessData {
                command: command.to_string(),
                output: String::new(),
                exit_status: None,
                receiver: rx,
                terminate_sender: Some(t_tx),
            },
        );
        Ok(self.counter)
    }

    pub fn stop(&mut self) {
        tracing::info!("Stop all running terminal commands");
        for (_, mut process) in self.processes.drain() {
            if process.exit_status.is_none() {
                if let Some(sender) = process.terminate_sender.take() {
                    sender.send(()).ok();
                }
            }
        }
    }

    pub fn poll(&mut self) -> Vec<AgentCommandStatus> {
        let mut modified_terminal_states = vec![];
        for (id, process) in self.processes.iter_mut() {
            if process.exit_status.is_none() {
                while let Ok(output) = process.receiver.try_recv() {
                    match output {
                        ProcessOutput::Exited(exit_status) => {
                            process.exit_status = Some(exit_status)
                        }
                        ProcessOutput::Output(str) => process.output += &str,
                        ProcessOutput::Error(str) => process.output += &str,
                    }
                    modified_terminal_states.push(AgentCommandStatus {
                        command_id: *id,
                        command: None,
                        output: process.output.clone(),
                        is_active: process.exit_status.is_none(),
                    });
                }
            }
        }
        modified_terminal_states
    }

    pub fn get_process(&self, id: usize) -> Option<(Option<ExitStatus>, &String)> {
        let process = self.processes.get(&id)?;
        Some((process.exit_status, &process.output))
    }

    pub fn processes(&self) -> impl Iterator<Item = (usize, Option<ExitStatus>, &String)> {
        self.processes
            .iter()
            .map(|(key, value)| (*key, value.exit_status, &value.command))
    }

    pub fn stop_process(&mut self, id: usize) -> Result<()> {
        let Some(process) = self.processes.get_mut(&id) else {
            return Ok(());
        };
        if process.exit_status.is_none() {
            if let Some(sender) = process.terminate_sender.take() {
                sender.send(()).ok();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn test_process_registry() {
        let mut registry = super::ProcessRegistry::new();
        registry
            .execute_command("npm start", ".\\target\\workspace")
            .await
            .unwrap();
        println!("{}: {}", registry.counter, registry.processes.len());
        println!("{:?}", registry.get_process(1));
        for _ in 0..5 {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            registry.poll();
            let (exit_code, output) = registry.get_process(1).unwrap();
            println!("{exit_code:?}, {output}");
            if exit_code.is_some() {
                break;
            }
        }
        println!("Stop processing");
        registry.stop_process(1).unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        registry.poll();
        let (exit_code, output) = registry.get_process(1).unwrap();
        println!("{exit_code:?}, {output}");
        println!("end");
    }
}
