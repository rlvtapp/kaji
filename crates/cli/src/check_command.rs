use super::*;
use crate::check_rules::{CheckDiagnostic, CheckSidecarOperation, check_api};

#[derive(Debug, Deserialize)]
struct CheckBaseline {
    diagnostics: Vec<CheckBaselineDiagnostic>,
}

#[derive(Debug, Deserialize)]
struct CheckBaselineDiagnostic {
    fingerprint: String,
}

#[derive(Serialize)]
struct CheckBaselineOutput {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: u8,
    diagnostics: Vec<CheckBaselineDiagnosticOutput>,
}

#[derive(Serialize)]
struct CheckBaselineDiagnosticOutput {
    fingerprint: String,
}

#[derive(Serialize)]
struct CheckJsonReport<'a> {
    version: u8,
    operations: usize,
    diagnostics: Vec<CheckJsonDiagnostic<'a>>,
    suppressed: Vec<CheckJsonDiagnostic<'a>>,
    summary: CheckJsonSummary,
}

#[derive(Serialize)]
struct CheckJsonDiagnostic<'a> {
    code: &'a str,
    severity: CheckSeverity,
    method: &'a str,
    path: &'a str,
    message: &'a str,
    hint: &'a str,
    fingerprint: String,
}

impl<'a> From<&'a CheckDiagnostic> for CheckJsonDiagnostic<'a> {
    fn from(diagnostic: &'a CheckDiagnostic) -> Self {
        Self {
            code: diagnostic.code,
            severity: diagnostic.severity,
            method: &diagnostic.method,
            path: &diagnostic.path,
            message: &diagnostic.message,
            hint: &diagnostic.hint,
            fingerprint: diagnostic.fingerprint(),
        }
    }
}

#[derive(Serialize)]
struct CheckJsonSummary {
    errors: usize,
    warnings: usize,
    suppressed: usize,
    failed: bool,
}

fn read_check_operations(artifacts: &Path) -> Result<Vec<CheckSidecarOperation>> {
    let index: BTreeMap<String, String> = serde_json::from_reader(
        std::fs::File::open(artifacts.join("operations.json"))
            .context("read compiled operation index")?,
    )
    .context("parse compiled operation index")?;
    let order: Vec<String> = serde_json::from_reader(
        std::fs::File::open(artifacts.join("operations-order.json"))
            .context("read compiled operation order")?,
    )
    .context("parse compiled operation order")?;
    order
        .into_iter()
        .map(|key| {
            let file = index
                .get(&key)
                .with_context(|| format!("compiled operation index is missing {key:?}"))?;
            serde_json::from_reader(
                std::fs::File::open(artifacts.join("operations").join(file))
                    .with_context(|| format!("read compiled operation {key:?}"))?,
            )
            .with_context(|| format!("parse compiled operation {key:?}"))
        })
        .collect()
}

pub(super) fn check(options: Check) -> Result<()> {
    let source = std::fs::canonicalize(&options.source)
        .with_context(|| format!("cannot read OpenAPI source {}", options.source.display()))?;
    if !source.is_file() {
        bail!("OpenAPI source must be a file")
    }
    let temporary = tempfile::tempdir().context("cannot create compiler working directory")?;
    let helper = compiler_path(options.compiler)?;
    let mut compiler = Command::new(&helper);
    compiler.arg("--out").arg(temporary.path()).arg(&source);
    // JSON is an API, so keep compiler progress off stdout where CI parsers and
    // agents expect exactly one document.
    if options.format == CheckFormat::Json {
        compiler.stdout(Stdio::null());
    }
    let status = compiler
        .status()
        .with_context(|| format!("cannot start OpenAPI compiler {}", helper.display()))?;
    if !status.success() {
        bail!("OpenAPI compiler failed ({status})")
    }
    let api = poolster_core::adapter::openapi_sidecar::load_operations(
        temporary.path(),
        "API".into(),
        "0.1.0".into(),
    )?;
    let source_operations = read_check_operations(temporary.path())?;
    let mut diagnostics = check_api(&api, &source_operations);
    for diagnostic in &mut diagnostics {
        if let Some(severity) = options.severity_overrides.get(diagnostic.code) {
            diagnostic.severity = *severity;
        }
    }

    if let Some(path) = &options.write_baseline {
        write_check_baseline(path, &diagnostics)?;
    }

    let baseline = match &options.baseline {
        Some(path) => load_check_baseline(path)?,
        None => BTreeSet::new(),
    };
    let (suppressed, reported): (Vec<_>, Vec<_>) =
        diagnostics.into_iter().partition(|diagnostic| {
            options.ignored_rules.contains(diagnostic.code)
                || baseline.contains(&diagnostic.fingerprint())
        });
    let errors = reported
        .iter()
        .filter(|diagnostic| diagnostic.severity == CheckSeverity::Error)
        .count();
    let warnings = reported.len() - errors;
    let failures = reported
        .iter()
        .filter(|diagnostic| options.fail_on.fails(diagnostic.severity))
        .count();

    match options.format {
        CheckFormat::Human => {
            if reported.is_empty() {
                print_check_passed(api.operations.len(), suppressed.len());
            } else {
                for diagnostic in &reported {
                    diagnostic.render();
                }
            }
        }
        CheckFormat::Json => {
            let report = CheckJsonReport {
                version: 1,
                operations: api.operations.len(),
                diagnostics: reported.iter().map(CheckJsonDiagnostic::from).collect(),
                suppressed: suppressed.iter().map(CheckJsonDiagnostic::from).collect(),
                summary: CheckJsonSummary {
                    errors,
                    warnings,
                    suppressed: suppressed.len(),
                    failed: failures > 0,
                },
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }

    if failures > 0 {
        bail!("check failed with {failures} issue(s)")
    }
    Ok(())
}

fn print_check_passed(operations: usize, suppressed: usize) {
    if suppressed == 0 {
        println!("check passed: {operations} operations are ready for generation");
    } else {
        println!(
            "check passed: {operations} operations are ready for generation ({suppressed} known issue(s) suppressed)"
        );
    }
}

pub(super) fn load_check_baseline(path: &Path) -> Result<BTreeSet<String>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot read check baseline {}", path.display()))?;
    let baseline: CheckBaseline = serde_json::from_reader(file)
        .with_context(|| format!("parse check baseline {}", path.display()))?;
    Ok(baseline
        .diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.fingerprint)
        .collect())
}

pub(super) fn write_check_baseline(path: &Path, diagnostics: &[CheckDiagnostic]) -> Result<()> {
    let document = CheckBaselineOutput {
        schema: "https://raw.githubusercontent.com/rlvtapp/kaji/main/schemas/v1/check-baseline.schema.json",
        version: 1,
        diagnostics: diagnostics
            .iter()
            .map(|diagnostic| CheckBaselineDiagnosticOutput {
                fingerprint: diagnostic.fingerprint(),
            })
            .collect(),
    };
    let document = serde_json::to_string_pretty(&document)?;
    std::fs::write(path, format!("{document}\n"))
        .with_context(|| format!("write check baseline {}", path.display()))
}
