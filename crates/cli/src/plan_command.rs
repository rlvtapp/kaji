use super::*;

pub(super) fn run(arguments: Vec<OsString>) -> Result<()> {
    let mut args = arguments.into_iter();
    let mut config = PathBuf::from("poolster.json");
    let mut output = None;
    let mut format = "text".to_owned();
    while let Some(arg) = args.next() {
        match arg.to_str().context("plan argument must be UTF-8")? {
            "--config" => config = PathBuf::from(args.next().context("--config needs a path")?),
            "--output" => {
                output = Some(PathBuf::from(args.next().context("--output needs a path")?))
            }
            "--format" => {
                let selected = args.next().context("--format needs html or json")?;
                match selected.to_str() {
                    Some("json" | "html" | "text") => {
                        format = selected.to_string_lossy().into_owned()
                    }
                    _ => bail!("--format needs text, html or json"),
                }
            }
            "--help" | "-h" => {
                println!(
                    "poolster plan --config poolster.json [--output plan.html] [--format text|html|json]\nInspects the declared Rust graph without loading input sources or generating packages."
                );
                return Ok(());
            }
            _ => bail!("unknown plan argument: {}", arg.to_string_lossy()),
        }
    }
    let options = generation_config::load_config_options(&config, ColorChoice::Auto, false, false)?;
    let native = options.native_input.is_some();
    let profiles = if let Some(input) = &options.native_input {
        native_profiles::build(&options, input)?
    } else {
        profiles(&options)?
    };
    let mut plan = profiles.plan(native);
    if let (Some(input), Some(packages)) = (&options.native_input, &options.config_packages) {
        for package in packages
            .iter()
            .filter(|p| !native_profiles::input_compatible(input, p))
        {
            plan.packages
                .push(poolster_core::engine::overview::PackagePlan {
                    path: package.path.clone(),
                    language: package.language.clone(),
                    status: "skipped".into(),
                    diagnostic: Some(format!(
                        "configured outputs cannot consume {} in this CLI pipeline",
                        input.format
                    )),
                    plugins: package
                        .plugins
                        .iter()
                        .enumerate()
                        .map(|(id, plugin)| poolster_core::engine::overview::PluginNode {
                            id,
                            kind: plugin.name.clone(),
                            label: plugin.name.clone(),
                            phase: "generate".into(),
                            order: None,
                            provides: vec![],
                            requires: vec![],
                            native: false,
                            handlers: vec!["configuration only; package skipped".into()],
                        })
                        .collect(),
                    edges: vec![],
                    stages: vec![],
                });
        }
    }
    let rendered = match format.as_str() {
        "json" => plan.to_json()?,
        "html" => plan.to_html()?,
        _ => plan.to_text(),
    };
    if let Some(path) = output {
        std::fs::write(&path, rendered).with_context(|| format!("write {}", path.display()))?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}
