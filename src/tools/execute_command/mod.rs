// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.

use std::collections::HashMap;
use std::process::ExitStatus;

use anyhow::Result;
use process_wrap::tokio::{TokioChildWrapper, TokioCommandWrap};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStderr, ChildStdin, ChildStdout};
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
    exit_status: Option<i32>,
    receiver: mpsc::UnboundedReceiver<ProcessOutput>,
    terminate_sender: Option<oneshot::Sender<()>>,
    input_sender: Option<mpsc::UnboundedSender<Vec<u8>>>,
}

enum ProcessOutput {
    Exited(Option<ExitStatus>),
    Output(String),
    Error(String),
}

struct ProcessRuntime {
    _process: Box<dyn TokioChildWrapper>,
    stdout: ChildStdout,
    stdin: ChildStdin,
    stderr: ChildStderr,
    sender: mpsc::UnboundedSender<ProcessOutput>,
    input_signal: mpsc::UnboundedReceiver<Vec<u8>>,
    terminate_signal: oneshot::Receiver<()>,
}

impl ProcessRuntime {
    pub async fn run(mut self) {
        use tokio::pin;

        let stdout = Self::handle_stdout(self.stdout, self.sender.clone());
        let stderr = Self::handle_stderr(self.stderr, self.sender.clone());
        let stdin = Self::handle_stdin(self.stdin, self.input_signal);

        let status = Box::into_pin(self._process.wait());
        pin!(stdout);
        pin!(stderr);
        pin!(stdin);

        let mut exit_status = None;
        tokio::select! {
            result = &mut stdout => {
                tracing::trace!("Stdout handler completed: {:?}", result);
                exit_status = Some(ExitStatus::default());
            }
            result = &mut stderr => {
                tracing::trace!("Stderr handler completed: {:?}", result);
            }
            // capture the status so we don't need to wait for a timeout
            result = status => {
                if let Ok(result) = result {
                    exit_status = Some(result);
                }
                tracing::trace!("Process exited with status: {:?}", result);
            }
            result = &mut stdin => {
                tracing::trace!("Stdin handler completed: {:?}", result);
            }
            _ = self.terminate_signal => {
                tracing::debug!("Receive terminal_signal");
                if self._process.start_kill().is_ok() {
                    if let Ok(status) = Box::into_pin(self._process.wait()).await {
                        exit_status = Some(status);
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

    async fn handle_stdin(mut stdin: ChildStdin, mut receiver: mpsc::UnboundedReceiver<Vec<u8>>) {
        while let Some(data) = receiver.recv().await {
            tracing::trace!("Writing data to stdin: {:?}", data);
            if let Err(e) = stdin.write_all(data.as_slice()).await {
                tracing::error!(error = ?e, "Error writing data to child process");
                break;
            }
            if let Err(e) = stdin.flush().await {
                tracing::error!(error = ?e, "Error flushing data to child process");
                break;
            }
        }
    }
}

impl ProcessRegistry {
    async fn spawn_process(
        &self,
        command: &str,
        cwd: &str,
    ) -> Result<(
        Box<dyn TokioChildWrapper>,
        ChildStdout,
        ChildStderr,
        ChildStdin,
    )> {
        let mut child = TokioCommandWrap::with_new(SHELL, |cmd| {
            cmd.current_dir(cwd)
                .stdin(std::process::Stdio::piped())
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

        let stdin = process
            .stdin()
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get stdin"))?;

        Ok((process, stdout, stderr, stdin))
    }

    pub async fn execute_command(&mut self, command: &str, cwd: &str) -> Result<usize> {
        self.counter = self.counter.saturating_add(1);
        let (process, stdout, stderr, stdin) = self.spawn_process(command, cwd).await?;
        let (tx, rx) = mpsc::unbounded_channel();
        let (t_tx, t_rx) = tokio::sync::oneshot::channel();
        let (in_tx, in_rx) = mpsc::unbounded_channel();

        let runtime = ProcessRuntime {
            _process: process,
            stdout,
            stderr,
            stdin,
            sender: tx,
            input_signal: in_rx,
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
                input_sender: Some(in_tx),
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
                            process.exit_status = Some(
                                exit_status
                                    .map(|s| s.code().unwrap_or_default())
                                    .unwrap_or(1),
                            )
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

    pub fn get_process(&self, id: usize) -> Option<(Option<i32>, &String)> {
        let process = self.processes.get(&id)?;
        Some((process.exit_status, &process.output))
    }

    pub fn processes(&self) -> impl Iterator<Item = (usize, Option<i32>, &String)> {
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

    pub fn send_data(&self, idx: usize, data: Vec<u8>) {
        if let Some(process) = self.processes.get(&idx) {
            if let Some(sender) = process.input_sender.as_ref() {
                sender.send(data).ok();
            }
        }
    }
}
