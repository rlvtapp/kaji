//! Explicit, transactional source formatting and final-byte quality checks.
//!
//! Formatter commands come from trusted generator configuration, never input
//! contracts. Arguments are passed directly without a shell. Unconfigured
//! generators retain their historical bytes; requesting quality checks is explicit.
use crate::{GeneratedFile, GeneratedTree};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn default_budget() -> usize {
    128 * 1024
}
fn default_timeout() -> u64 {
    120
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFormatter {
    pub program: String,
    /// Replace {file} with the temporary source path. The formatter edits it in place.
    pub arguments: Vec<String>,
    #[serde(rename = "versionArguments", alias = "version_arguments")]
    pub version_arguments: Vec<String>,
    /// Exact trimmed version response, not a substring that accepts another version.
    #[serde(rename = "expectedVersion", alias = "expected_version")]
    pub expected_version: String,
    /// File extensions without a dot, e.g. "php" or "rs".
    pub extensions: Vec<String>,
    #[serde(
        default = "default_timeout",
        rename = "timeoutSeconds",
        alias = "timeout_seconds"
    )]
    pub timeout_seconds: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceQuality {
    Formatted {
        formatter: SourceFormatter,
        #[serde(
            default = "default_budget",
            rename = "maxFileBytes",
            alias = "max_file_bytes"
        )]
        max_file_bytes: usize,
    },
    /// An explicit diagnostic, not a formatting/conformance claim.
    Unformatted { reason: String },
}

impl SourceQuality {
    /// Validate configuration without invoking the trusted formatter executable.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Unformatted { reason } => ensure!(
                !reason.trim().is_empty(),
                "unformatted source quality requires a reason"
            ),
            Self::Formatted {
                formatter,
                max_file_bytes,
            } => {
                formatter.validate()?;
                ensure!(
                    *max_file_bytes > 0,
                    "source quality file budget must be positive"
                );
            }
        }
        Ok(())
    }
    pub fn apply(&self, tree: &mut GeneratedTree, language: &str) -> Result<()> {
        self.validate()?;
        let mut staged = tree.clone();
        let report = match self {
            Self::Unformatted { reason } => {
                ensure!(
                    !reason.trim().is_empty(),
                    "unformatted source quality requires a reason"
                );
                serde_json::json!({"version":1,"language":language,"status":"unformatted","reason":reason})
            }
            Self::Formatted {
                formatter,
                max_file_bytes,
            } => {
                formatter.validate()?;
                ensure!(
                    *max_file_bytes > 0,
                    "source quality file budget must be positive"
                );
                let temporary = tempfile::tempdir().context("create source formatter workspace")?;
                let version = formatter.run(&formatter.version_arguments, temporary.path())?;
                ensure!(
                    version.trim() == formatter.expected_version.trim(),
                    "source formatter version mismatch: expected {:?}, got {:?}",
                    formatter.expected_version,
                    version.trim()
                );
                let paths = staged
                    .iter()
                    .filter(|(path, _)| {
                        path.extension().is_some_and(|ext| {
                            formatter
                                .extensions
                                .iter()
                                .any(|allowed| ext == allowed.as_str())
                        })
                    })
                    .map(|(path, _)| path.to_owned())
                    .collect::<Vec<_>>();
                ensure!(
                    !paths.is_empty(),
                    "source formatter matched no generated files for {language}"
                );
                let mut files = Vec::new();
                let mut diagnostic: serde_json::Value = staged
                    .get(".poolster/source-layout-diagnostics.json")
                    .map(serde_json::from_str)
                    .transpose()?
                    .unwrap_or_else(|| serde_json::json!([]));
                ensure!(
                    diagnostic.is_array(),
                    "source layout diagnostics must be an array"
                );
                for (index, path) in paths.iter().enumerate() {
                    let source = staged.get(path).unwrap();
                    let directory = temporary.path().join(format!("source-{index}"));
                    fs::create_dir(&directory)?;
                    let file = directory.join(path.file_name().unwrap());
                    fs::write(&file, source)?;
                    let arguments = formatter
                        .arguments
                        .iter()
                        .map(|arg| arg.replace("{file}", &file.to_string_lossy()))
                        .collect::<Vec<_>>();
                    formatter
                        .run(&arguments, temporary.path())
                        .with_context(|| format!("format {}", path.display()))?;
                    let formatted = fs::read_to_string(&file)
                        .with_context(|| format!("read formatted {}", path.display()))?;
                    ensure!(
                        !formatted.trim().is_empty() || source.trim().is_empty(),
                        "formatter erased {}",
                        path.display()
                    );
                    ensure!(
                        !formatted.contains('\r'),
                        "formatter emitted non-LF line endings for {}",
                        path.display()
                    );
                    ensure!(
                        formatted.is_empty() || formatted.ends_with('\n'),
                        "formatter omitted final newline for {}",
                        path.display()
                    );
                    let mut atomic_exception = false;
                    if let Some(entry) = diagnostic
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|entry| entry["path"].as_str() == path.to_str())
                    {
                        entry["bytes"] = formatted.len().into();
                        entry["max_file_bytes"] = (*max_file_bytes).into();
                        atomic_exception = true;
                    }
                    ensure!(
                        formatted.len() <= *max_file_bytes || atomic_exception,
                        "formatted {} exceeds {} bytes without an existing atomic-declaration diagnostic; split it at semantic boundaries",
                        path.display(),
                        max_file_bytes
                    );
                    files.push(serde_json::json!({"path":path,"bytes":formatted.len(),"atomic_exception":atomic_exception}));
                    staged.replace(GeneratedFile::new(path, formatted)?)?;
                }
                if staged
                    .get(".poolster/source-layout-diagnostics.json")
                    .is_some()
                {
                    staged.replace(GeneratedFile::new(
                        ".poolster/source-layout-diagnostics.json",
                        format!("{}\n", serde_json::to_string_pretty(&diagnostic)?),
                    )?)?;
                }
                serde_json::json!({"version":1,"language":language,"status":"formatted","formatter_version":formatter.expected_version,"max_file_bytes":max_file_bytes,"files":files})
            }
        };
        let report = GeneratedFile::new(
            ".poolster/source-quality.json",
            format!("{}\n", serde_json::to_string_pretty(&report)?),
        )?;
        ensure!(
            staged.get(&report.path).is_none(),
            "source quality report path is reserved"
        );
        staged.insert(report)?;
        staged.set_owner(".poolster/source-quality.json", "source-quality")?;
        *tree = staged;
        Ok(())
    }
}
impl SourceFormatter {
    fn validate(&self) -> Result<()> {
        ensure!(
            !self.program.trim().is_empty(),
            "source formatter program is required"
        );
        ensure!(
            !self.expected_version.trim().is_empty(),
            "source formatter version must be pinned"
        );
        ensure!(
            !self.version_arguments.is_empty(),
            "source formatter version command is required"
        );
        ensure!(
            self.arguments.iter().any(|arg| arg.contains("{file}")),
            "source formatter arguments must include {{file}}"
        );
        ensure!(
            self.timeout_seconds > 0 && self.timeout_seconds <= 3600,
            "source formatter timeout must be 1..3600 seconds"
        );
        ensure!(
            !self.extensions.is_empty()
                && self
                    .extensions
                    .iter()
                    .all(|ext| !ext.is_empty()
                        && ext.bytes().all(|byte| byte.is_ascii_alphanumeric())),
            "source formatter requires plain file extensions"
        );
        Ok(())
    }
    fn run(&self, arguments: &[String], directory: &std::path::Path) -> Result<String> {
        let stdout = directory.join("stdout.log");
        let stderr = directory.join("stderr.log");
        let mut child = Command::new(&self.program)
            .args(arguments)
            .current_dir(directory)
            .stdin(Stdio::null())
            .stdout(fs::File::create(&stdout)?)
            .stderr(fs::File::create(&stderr)?)
            .spawn()
            .with_context(|| format!("start source formatter {}", self.program))?;
        let start = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if start.elapsed() > Duration::from_secs(self.timeout_seconds) {
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!(
                    "source formatter timed out after {} seconds",
                    self.timeout_seconds
                );
            }
            thread::sleep(Duration::from_millis(20));
        };
        let read_bounded = |path: &std::path::Path| -> Result<String> {
            use std::io::Read;
            let mut bytes = Vec::new();
            fs::File::open(path)?
                .take(64 * 1024)
                .read_to_end(&mut bytes)?;
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        };
        let output = read_bounded(&stdout)?;
        ensure!(
            status.success(),
            "source formatter failed ({status}): {}",
            read_bounded(&stderr)?
        );
        Ok(output)
    }
}

#[cfg(test)]
mod tests;
