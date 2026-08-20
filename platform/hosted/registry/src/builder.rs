//! Hermetic sandbox that rebuilds published source with the pinned toolchain.
//!
//! Each attempt materialises the canonical archive into a fresh directory,
//! runs the declared command with a scrubbed environment and no network, and
//! returns only the bytes found at the declared artifact path. Nothing about
//! the request influences the command: it comes from the published build plan,
//! and the plan's builder image must be the image this host is pinned to.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use layerx_programs::{hex, BuildAttempt, BuildRefusal, BuildRunner, SourceArchive};

const BUILD_LOG: &str = "layerx-build.log";
const POLL_MILLISECONDS: u64 = 100;
const LOG_TAIL: usize = 1_024;

/// Sandbox that executes exactly one declared build command per attempt.
#[derive(Clone, Debug)]
pub struct HermeticBuilder {
    workspace: PathBuf,
    builder_image_digest: [u8; 32],
    path: String,
    timeout_seconds: u64,
}

impl HermeticBuilder {
    /// Binds the sandbox to the builder image this host is pinned to.
    ///
    /// # Errors
    ///
    /// Refuses an unpinned builder image, an empty search path, an unbounded
    /// timeout and an unusable workspace.
    pub fn new(
        workspace: PathBuf,
        builder_image_digest: [u8; 32],
        path: String,
        timeout_seconds: u64,
    ) -> Result<Self, String> {
        if builder_image_digest == [0; 32] {
            return Err("the pinned builder image digest is required".to_owned());
        }
        if path.is_empty() || timeout_seconds == 0 {
            return Err("a builder search path and build timeout are required".to_owned());
        }
        fs::create_dir_all(&workspace).map_err(|error| {
            format!(
                "could not open the build workspace {}: {error}",
                workspace.display()
            )
        })?;
        Ok(Self {
            workspace,
            builder_image_digest,
            path,
            timeout_seconds,
        })
    }

    /// Returns the builder image digest every published plan must declare.
    #[must_use]
    pub const fn builder_image_digest(&self) -> [u8; 32] {
        self.builder_image_digest
    }

    fn sandbox(&self, attempt: &BuildAttempt<'_>) -> Result<PathBuf, BuildRefusal> {
        let root = self.workspace.join(format!(
            "{}-{}",
            hex::encode(&attempt.archive.digest()),
            attempt.attempt
        ));
        if root.exists() {
            fs::remove_dir_all(&root).map_err(unavailable)?;
        }
        fs::create_dir_all(&root).map_err(unavailable)?;
        materialize(&root, attempt.archive)?;
        Ok(root)
    }

    fn execute(&self, attempt: &BuildAttempt<'_>, root: &Path) -> Result<(), BuildRefusal> {
        let (program, arguments) = attempt
            .plan
            .environment
            .command
            .split_first()
            .ok_or(BuildRefusal::InvalidPlan)?;
        let log_path = root.join(BUILD_LOG);
        let log = File::create(&log_path).map_err(unavailable)?;
        let mut child = Command::new(program)
            .args(arguments)
            .current_dir(root)
            .env_clear()
            .env("PATH", &self.path)
            .env("HOME", root)
            .env("TMPDIR", root)
            .env("CARGO_HOME", root.join(".layerx-cargo"))
            .env("RUSTUP_HOME", root.join(".layerx-rustup"))
            .env("CARGO_TERM_COLOR", "never")
            .env("CARGO_NET_OFFLINE", "true")
            .env(
                "SOURCE_DATE_EPOCH",
                attempt.plan.environment.source_date_epoch.to_string(),
            )
            .env("TZ", "UTC")
            .env("LC_ALL", "C")
            .env(
                "RUSTFLAGS",
                format!("--remap-path-prefix={}=/layerx/source", root.display()),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(log))
            .spawn()
            .map_err(unavailable)?;
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(self.timeout_seconds))
            .ok_or_else(|| BuildRefusal::SandboxUnavailable {
                reason: "declared build timeout is out of range".to_owned(),
            })?;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(BuildRefusal::BuilderFailed {
                            reason: format!(
                                "pinned build exceeded {} seconds",
                                self.timeout_seconds
                            ),
                        });
                    }
                    thread::sleep(Duration::from_millis(POLL_MILLISECONDS));
                }
                Err(error) => return Err(unavailable(error)),
            }
        };
        if status.success() {
            Ok(())
        } else {
            Err(BuildRefusal::BuilderFailed {
                reason: format!("{status}: {}", tail(&log_path)),
            })
        }
    }
}

impl BuildRunner for HermeticBuilder {
    fn run(&self, attempt: &BuildAttempt<'_>) -> Result<Vec<u8>, BuildRefusal> {
        if attempt.plan.environment.builder_image_digest != self.builder_image_digest {
            return Err(BuildRefusal::BuilderImageMismatch);
        }
        let root = self.sandbox(attempt)?;
        let produced = self.execute(attempt, &root).and_then(|()| {
            fs::read(root.join(&attempt.plan.artifact_path)).map_err(|_| {
                BuildRefusal::MissingArtifact {
                    path: attempt.plan.artifact_path.clone(),
                }
            })
        });
        let _ = fs::remove_dir_all(&root);
        produced
    }
}

fn materialize(root: &Path, archive: &SourceArchive) -> Result<(), BuildRefusal> {
    for file in archive.files() {
        let target = root.join(&file.path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(unavailable)?;
        }
        fs::write(&target, &file.content).map_err(unavailable)?;
        if file.executable {
            make_executable(&target)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), BuildRefusal> {
    use std::os::unix::fs::PermissionsExt as _;

    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(unavailable)
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), BuildRefusal> {
    Err(BuildRefusal::SandboxUnavailable {
        reason: "executable source files require a POSIX build host".to_owned(),
    })
}

fn tail(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_default();
    let start = bytes.len().saturating_sub(LOG_TAIL);
    String::from_utf8_lossy(bytes.get(start..).unwrap_or_default())
        .replace(['\n', '\r', '"'], " ")
        .trim()
        .to_owned()
}

fn unavailable(error: io::Error) -> BuildRefusal {
    BuildRefusal::SandboxUnavailable {
        reason: error.to_string(),
    }
}
