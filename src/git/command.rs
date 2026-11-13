//! Running `git`: one argument vector per command, never a shell.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

/// The oldest git the parsers are written for.
pub const MIN_VERSION: (u32, u32) = (2, 30);

/// How to run git: the program and extra environment. Cheap to clone and
/// safe to send to background threads.
#[derive(Clone, Debug)]
pub struct Git {
    program: OsString,
    env: Vec<(OsString, OsString)>,
}

impl Default for Git {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether a command may take git's optional locks. Status refreshes run
/// with `GIT_OPTIONAL_LOCKS=0` so they never block the user's own git.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
}

impl Git {
    /// The `git` on `PATH`, with the user's own configuration.
    pub fn new() -> Self {
        Self {
            program: "git".into(),
            env: Vec::new(),
        }
    }

    /// The same git with one more environment variable for every command.
    pub fn with_env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    fn command(&self, repo: Option<&Path>, args: &[&OsStr], access: Access) -> Command {
        let mut command = Command::new(&self.program);
        if let Some(repo) = repo {
            command.arg("-C").arg(repo);
        }
        command
            .args(args)
            // Fail instead of waiting for a terminal the app does not have.
            .env("GIT_TERMINAL_PROMPT", "0")
            // Parseable progress and messages.
            .env("LC_ALL", "C")
            .env("LANGUAGE", "")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if access == Access::Read {
            command.env("GIT_OPTIONAL_LOCKS", "0");
        }
        for (key, value) in &self.env {
            command.env(key, value);
        }
        #[cfg(windows)]
        {
            // A GUI app should not flash a console window for each command.
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command
    }

    fn spawn(&self, repo: Option<&Path>, args: &[&OsStr], access: Access) -> Result<Child, Error> {
        self.command(repo, args, access)
            .spawn()
            .map_err(|error| match error.kind() {
                io::ErrorKind::NotFound => Error::NotInstalled,
                _ => Error::Io(error.to_string()),
            })
    }

    /// Run `git -C repo args…` and return its stdout. A non-zero exit is an
    /// [`Error::Failed`] carrying stderr.
    pub fn run(&self, repo: &Path, args: &[&OsStr], access: Access) -> Result<Vec<u8>, Error> {
        self.run_with_input(repo, args, access, None)
    }

    /// As [`run`](Self::run), writing `input` to the command's stdin.
    pub fn run_with_input(
        &self,
        repo: &Path,
        args: &[&OsStr],
        access: Access,
        input: Option<&[u8]>,
    ) -> Result<Vec<u8>, Error> {
        let output = self.output(Some(repo), args, access, input)?;
        if output.code == Some(0) {
            Ok(output.stdout)
        } else {
            Err(output.failure(args))
        }
    }

    /// Run git and return its output whatever the exit code.
    pub fn output(
        &self,
        repo: Option<&Path>,
        args: &[&OsStr],
        access: Access,
        input: Option<&[u8]>,
    ) -> Result<Output, Error> {
        let mut command = self.command(repo, args, access);
        if input.is_some() {
            command.stdin(Stdio::piped());
        }
        let mut child = command.spawn().map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => Error::NotInstalled,
            _ => Error::Io(error.to_string()),
        })?;
        if let Some(input) = input {
            let mut stdin = child.stdin.take().expect("stdin is piped");
            stdin
                .write_all(input)
                .map_err(|error| Error::Io(error.to_string()))?;
        }
        let output = child
            .wait_with_output()
            .map_err(|error| Error::Io(error.to_string()))?;
        Ok(Output {
            code: output.status.code(),
            stdout: output.stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    /// Run a long command (fetch, pull, push), passing each stderr line or
    /// carriage-return-separated progress update to `on_line`. Setting
    /// `cancel` kills the command. Returns the full stderr on success.
    pub fn run_streaming(
        &self,
        repo: &Path,
        args: &[&OsStr],
        cancel: &Cancel,
        mut on_line: impl FnMut(&str),
    ) -> Result<String, Error> {
        // Cancelled before it started: do not start it.
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let mut child = self.spawn(Some(repo), args, Access::Write)?;
        let mut stderr = child.stderr.take().expect("stderr is piped");
        let mut stdout = child.stdout.take().expect("stdout is piped");
        let (sender, receiver) = mpsc::channel::<String>();
        let reader = thread::spawn(move || {
            let mut pending = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                match stderr.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        for &byte in &buffer[..read] {
                            if byte == b'\n' || byte == b'\r' {
                                if !pending.is_empty() {
                                    let line = String::from_utf8_lossy(&pending).into_owned();
                                    pending.clear();
                                    if sender.send(line).is_err() {
                                        return;
                                    }
                                }
                            } else {
                                pending.push(byte);
                            }
                        }
                    }
                }
            }
            if !pending.is_empty() {
                let _ = sender.send(String::from_utf8_lossy(&pending).into_owned());
            }
        });
        // Drain stdout so a chatty command cannot fill the pipe and stall.
        let drain = thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = stdout.read_to_end(&mut sink);
        });
        let mut log = String::new();
        let mut cancelled = false;
        loop {
            match receiver.recv_timeout(Duration::from_millis(50)) {
                Ok(line) => {
                    on_line(&line);
                    log.push_str(&line);
                    log.push('\n');
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            if cancel.is_cancelled() && !cancelled {
                cancelled = true;
                let _ = child.kill();
            }
        }
        let _ = reader.join();
        let _ = drain.join();
        let status = child.wait().map_err(|error| Error::Io(error.to_string()))?;
        if cancelled {
            Err(Error::Cancelled)
        } else if status.success() {
            Ok(log)
        } else {
            Err(Error::Failed {
                command: describe(args),
                code: status.code(),
                stderr: log,
            })
        }
    }

    /// The installed git's version, as `(major, minor)`.
    pub fn version(&self) -> Result<(u32, u32), Error> {
        let output = self.output(None, &[OsStr::new("--version")], Access::Read, None)?;
        let text = String::from_utf8_lossy(&output.stdout);
        parse_version(&text).ok_or_else(|| Error::Io(format!("unexpected `git --version`: {text}")))
    }
}

/// What a finished command printed.
#[derive(Debug)]
pub struct Output {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

impl Output {
    pub fn failure(self, args: &[&OsStr]) -> Error {
        Error::Failed {
            command: describe(args),
            code: self.code,
            stderr: self.stderr,
        }
    }
}

/// A flag that cancels a running [`Git::run_streaming`].
#[derive(Clone, Debug, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// `git` is not on `PATH`.
    NotInstalled,
    /// The command ran and exited with an error.
    Failed {
        command: String,
        code: Option<i32>,
        stderr: String,
    },
    Cancelled,
    /// The process could not be started or its output read, or git printed
    /// something the parsers do not understand.
    Io(String),
}

impl Error {
    /// git's own message, for the status bar.
    pub fn summary(&self) -> String {
        match self {
            Error::NotInstalled => "git was not found. Install git and restart the app.".into(),
            Error::Failed { stderr, .. } => {
                let lines: Vec<&str> = stderr
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .collect();
                lines
                    .iter()
                    .rev()
                    .find(|line| line.starts_with("fatal:") || line.starts_with("error:"))
                    .or(lines.last())
                    .map(|line| line.to_string())
                    .unwrap_or_else(|| "git failed".into())
            }
            Error::Cancelled => "Cancelled".into(),
            Error::Io(message) => message.clone(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Failed {
                command,
                code,
                stderr,
            } => write!(f, "{command} failed ({code:?}): {}", stderr.trim()),
            other => f.write_str(&other.summary()),
        }
    }
}

impl std::error::Error for Error {}

fn describe(args: &[&OsStr]) -> String {
    let mut text = String::from("git");
    for arg in args {
        text.push(' ');
        text.push_str(&arg.to_string_lossy());
    }
    text
}

/// `git version 2.45.1.windows.1` → `(2, 45)`.
pub fn parse_version(text: &str) -> Option<(u32, u32)> {
    let version = text.trim().strip_prefix("git version ")?;
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts
        .next()?
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()?;
    Some((major, minor))
}

/// Builds argument vectors from a mix of strings and paths.
#[macro_export]
#[doc(hidden)]
macro_rules! git_args {
    ($($arg:expr),* $(,)?) => {
        [$(::std::ffi::OsStr::new($arg)),*]
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_parse() {
        assert_eq!(
            parse_version("git version 2.55.0.windows.5\n"),
            Some((2, 55))
        );
        assert_eq!(parse_version("git version 2.30.1"), Some((2, 30)));
        assert_eq!(parse_version("git version 2.43.0-rc1"), Some((2, 43)));
        assert_eq!(parse_version("hg 6"), None);
    }

    #[test]
    fn the_summary_is_the_fatal_line() {
        let error = Error::Failed {
            command: "git pull".into(),
            code: Some(128),
            stderr: "hint: something\nfatal: Not possible to fast-forward, aborting.\n".into(),
        };
        assert_eq!(
            error.summary(),
            "fatal: Not possible to fast-forward, aborting."
        );
    }
}
