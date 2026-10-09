use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::Instant;

use anyhow::{Context, Result, bail, ensure};
use check_rules::CHECK_RULES;
use cli_action_types::*;
use cli_args::parse;
use generation_artifacts::{add_typescript_artifact_dependencies, append_config_artifacts};
use generation_config::{config_path, generate_from_config, init_config};
use generation_direct::generate;
use generation_lock::{
    GENERATION_LOCK_PATH, GENERATION_LOCK_VERSION, GenerationReplayLock, UpdateInputLock,
    UpdateLock, generation_lock,
};
#[cfg(test)]
use mock_http::{
    mock_path_matches, mock_path_parameters, mock_path_specificity, mock_response_body,
    mock_target_parts,
};
use mock_server::serve_mock;
use openapi_sources::{compiler_source_origin, download_openapi, remote_spec_url};
use poolster::ts::artifacts::{
    ArtifactOptions, McpToolManifest, ReDoc, TypeScriptCypress, TypeScriptFaker, TypeScriptMsw,
    TypeScriptReactQuery, TypeScriptSwr, TypeScriptVueQuery, TypeScriptZod,
};
use poolster::{
    SdkClientStyle, csharp, dotnet, elixir, go, java, mock, php, postman, prelude::*, python, ruby,
    rust, rust_cli, swift, symfony, terraform, ts, ts_cli,
};
use poolster_core::{Api, GeneratedFile, GeneratedTree};
use profile_config::*;
use recipe_types::*;
use runtime_support::*;
use runtime_types::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use typescript_profile::typescript_profile;
#[cfg(test)]
use update_command::replay_input_is_unchanged;
use update_command::update;

mod check_command;
mod check_rules;
mod cli_action_types;
mod cli_args;
mod config_defaults;
mod config_profile_cli;
mod config_profile_other;
mod config_profile_sdk;
mod contract;
mod credentials;
mod eject;
mod generation_artifacts;
mod generation_config;
mod generation_direct;
mod generation_lock;
mod generation_native;
mod mcp;
mod migration;
mod mock_http;
mod mock_server;
mod native_graphql_addons;
mod native_profiles;
mod openapi_sources;
mod plan_command;
mod profile_config;
mod recipe_exporters;
mod recipe_types;
mod registry;
mod runtime_support;
mod runtime_types;
mod sdk_automation;
mod sdk_doctor;
mod sdk_install;
mod sdk_status;
mod show_command;
mod typescript_profile;
mod update_command;

const HELP: &str = "Poolster — native contract package generator

Usage:
  poolster plan --config poolster.json [--format text|json|html] [--output PATH]
  poolster migrate [project-or-config] [--input <file>] [--output <new-directory>] [--strict]
  poolster init [--config <file>] [--input <openapi-file>] [--output <directory>]
  poolster generate                         # reads ./poolster.json
  poolster generate --config <file>
  poolster generate <schema.graphql> --input-format graphql --operation <operations.graphql> --output <directory> --language typescript
  poolster generate <openapi-file> --output <directory> --language <target>...
  poolster generate --artifacts <directory> --output <directory> --language <target>...
  poolster mcp <openapi-file> --base-url <url>
  poolster mcp generator
  poolster mock serve <openapi-file> [--port <port>]
  poolster contract plugins [--format human|json]
  poolster contract inspect <file> --input-format <format> [--provider <id>] [--format human|json]
  poolster check <openapi-file> [--format human|json]
  poolster show <openapi-file> [--include-path <pattern>] [--exclude-path <pattern>]
  poolster update [--output <directory>] [--force]
  poolster auth <login|logout|status> ...
  poolster discover <query> [--limit <count>] [--format human|json]
  poolster download <api-id> --output <openapi-file> [--version <version>]
  poolster languages
  poolster eject --language <target> --out <source-workspace>
  poolster sdk <init|sync|app|list|run|diff|pr|releases|connect|install|status|doctor|inspect> ...
  poolster --version

Config commands:
  init                                  Write a starter poolster.json; never overwrites it
  generate                              Read the config by default
      --config <file>                   Read a specific config file
      --color <mode>                    auto (default), always, or never

Generate options (both modes):
      --check                          Report drift without writing output
      --format human|json              Change report format
  -o, --output <directory>             Output root (required)
  -l, --language <target,...>          Repeatable; use all for every SDK (required)
      --input-format <format>         Native format: graphql, protobuf, asyncapi or arazzo
      --provider <id>                 Registered native input provider
      --operation <file>              Repeatable GraphQL operation documents
      --import-root <directory>       Repeatable provider import roots
      --broker-config <file>          Provider broker configuration as JSON
      --workflow-source <name=path>   Provider workflow source resolution
      --module <path>                Protobuf Go module import path
      --protoc <file>                 Official protoc toolchain executable
      --protoc-gen-go <file>          Official Go message compiler plugin
      --protoc-gen-go-grpc <file>     Official Go gRPC compiler plugin
      --go-package <file=mapping>     Repeatable Go import/package mappings
      --name <name>                   API name (default: API)
      --sdk-version <version>         Generated package version (default: 0.1.0)
      --client-style <style>          namespaced (default), idiomatic, or flat
      --typescript-transport <kind>   fetch (default) or axios
      --raw-sdk                     Raw GraphQL operation surface
      --typescript-surface <surface>  client (default) or raw
      --typescript-client-name <name> TypeScript client class name
      --jobs <count>                  Go emission workers (default: bounded auto)
      --artifacts <directory>         Reuse compiled OpenAPI JSON artifacts
      --openapi-compiler <file>       Override bundled poolster-openapi executable
      --include-path <pattern>        Generate only matching OpenAPI paths; repeatable
      --exclude-path <pattern>        Omit matching OpenAPI paths; repeatable
  -h, --help                          Show help

Targets: postman, terraform, rust, rust-cli, typescript, typescript-cli, go, python, php, symfony, java, csharp, dotnet (legacy alias), elixir, ruby, swift

MCP commands:
  mcp                                   Serve an OpenAPI document as MCP tools over stdio
      --base-url <url>                  API origin used when a tool is called (required)
      --openapi-compiler <file>         Override bundled poolster-openapi executable
  mcp generator                         Serve Poolster generation controls as MCP tools over stdio

Mock commands:
  mock serve                            Serve OpenAPI-derived happy-path responses without Docker
      --port <port>                     Local port (default: 4010)
      --openapi-compiler <file>         Override bundled poolster-openapi executable

Contract commands:
  check                                 Find API-contract issues that make generated SDKs and CLIs awkward
      --openapi-compiler <file>         Override bundled poolster-openapi executable
      --format <format>                  human (default) or json for automation
      --severity <rule=level>            Override a rule as warning or error; repeatable
      --fail-on <level>                  error (default), warning, or none
      --baseline <file>                  Suppress matching, known diagnostic fingerprints
      --write-baseline <file>            Record current diagnostics as a baseline
      --ignore <rule>                    Suppress a rule entirely; repeatable

Update commands:
  show <openapi-file>                    Inspect the generated path and operation tree
      --include-path <pattern>            Repeatable path filter
      --exclude-path <pattern>            Repeatable path exclusion
      --format <format>                   human (default) or json for automation
  update                                 Replay direct-generation lock files below output
      --output <directory>                Search root (default: current directory)
      --force                             Regenerate even when local input is unchanged

Auth commands:
  auth login <profile> --token-env <name> Register a named, environment-backed token
  auth logout <profile>                   Remove a named token profile
  auth status                             List profiles without exposing tokens

Registry commands:
  discover <query>                       Search the public OpenAPI directory
      --limit <count>                    Results to return (default: 20)
      --format <format>                  human (default) or json for automation
  download <api-id>                      Download its preferred OpenAPI version
      --version <version>                Select a directory version explicitly
      --output <openapi-file>            Destination; must not already exist

Each target is written to its own subdirectory. Owned generated files are updated;
custom starter files and unrelated files are preserved. Generation accepts local files,
HTTPS URLs, or a remote input object in config. The registry commands use APIs.guru.
The npm distribution bundles both native executables; Rust and Go are not required.
";

const LANGUAGES: &[&str] = &[
    "rust",
    "rust-cli",
    "typescript",
    "typescript-cli",
    "go",
    "python",
    "php",
    "symfony",
    "java",
    "csharp",
    "dotnet",
    "elixir",
    "ruby",
    "swift",
    "postman",
    "terraform",
];

// `all` intentionally remains the established shortcut for SDK packages. A
// generated executable needs product-specific configuration, so users select
// `typescript-cli` explicitly when they want one.
const SDK_LANGUAGES: &[&str] = &[
    "rust",
    "typescript",
    "go",
    "python",
    "php",
    "java",
    "csharp",
    "elixir",
    "ruby",
    "swift",
];

fn main() -> ExitCode {
    let action = match parse(env::args_os().skip(1)) {
        Ok(action) => action,
        Err(error) => {
            eprintln!("poolster: {error:#}");
            return ExitCode::from(2);
        }
    };
    let result = match action {
        Action::Help => {
            print!("{HELP}");
            Ok(())
        }
        Action::Version => {
            println!("poolster {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Action::Languages => {
            println!("{}", LANGUAGES.join("\n"));
            Ok(())
        }
        Action::Show(options) => show_command::show(options),
        Action::Update(options) => update(options),
        Action::Auth(options) => auth(options),
        Action::Discover(options) => openapi_sources::discover(options),
        Action::Download(options) => openapi_sources::download(options),
        Action::Init(init) => init_config(init),
        Action::Mcp(options) => serve_mcp(options),
        Action::McpGenerator => mcp::serve_generator(),
        Action::MockServe(options) => serve_mock(options),
        Action::Check(options) => check_command::check(options),
        Action::Generate(options) => generate(*options),
        Action::Sdk(options) => sdk_automation::run(options),
        Action::Eject(arguments) => eject::run(arguments),
        Action::Migrate(arguments) => migration::run(arguments),
        Action::Plan(arguments) => plan_command::run(arguments),
        Action::Contract(arguments) => contract::run(arguments),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("poolster: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests;
