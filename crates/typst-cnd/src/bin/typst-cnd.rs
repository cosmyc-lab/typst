use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::builder::ValueParser;
use clap::{ArgAction, Parser, Subcommand};
use ecow::eco_vec;
use typst::compile;
use typst::diag::{SourceResult, error};
use typst::foundations::{Dict, IntoValue};
use typst_cnd::{CndDocument, cnd_from_document, cnd_to_json, world};
use typst_syntax::Span;

#[derive(Parser)]
#[command(name = "typst-cnd", about = "Compile Typst sources into CND JSON")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile a `.typ` file into a `.cnd` file.
    Compile {
        /// Input Typst file.
        input: PathBuf,
        /// Output `.cnd` path.
        #[arg(short, long)]
        output: PathBuf,
        /// Add a string key-value pair visible through `sys.inputs`.
        ///
        /// Repeatable. A key given here overrides the same key coming from
        /// `--inputs-file`.
        #[arg(
            short = 'i',
            long = "input",
            value_name = "key=value",
            action = ArgAction::Append,
            value_parser = ValueParser::new(parse_sys_input_pair),
        )]
        input_pairs: Vec<(String, String)>,
        /// Load a JSON object of string values into `sys.inputs`.
        ///
        /// Every value must be a JSON string — typically itself a
        /// serialized JSON document, decoded again on the Typst side. Use
        /// this instead of repeated `--input` flags when the data would
        /// not fit comfortably on the command line. A key also set via
        /// `--input` is overridden by that flag, not by this file.
        #[arg(long = "inputs-file", value_name = "PATH")]
        inputs_file: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(errors) => {
            for error in errors {
                eprintln!("{error:?}");
            }
            ExitCode::FAILURE
        }
    }
}

fn run() -> SourceResult<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Compile { input, output, input_pairs, inputs_file } => {
            let inputs = build_inputs(&input_pairs, inputs_file.as_deref())?;
            compile_file(&input, &output, inputs)
        }
    }
}

fn compile_file(input: &Path, output: &Path, inputs: Dict) -> SourceResult<()> {
    let world = world::CndWorld::new_with_inputs(input, inputs).map_err(|err| {
        eco_vec![error!(Span::detached(), "failed to initialize world: {err}")]
    })?;

    let warned = compile::<CndDocument>(&world);
    for warning in &warned.warnings {
        eprintln!("warning: {warning:?}");
    }

    let document = warned.output?;
    let cnd =
        cnd_from_document(&document, world::source_info(&world), world::built_at_now());
    let json = cnd_to_json(&cnd)?;
    std::fs::write(output, json).map_err(|err| {
        eco_vec![error!(Span::detached(), "failed to write output file: {err}")]
    })?;

    Ok(())
}

/// Parses key/value pairs split by the first equal sign.
///
/// Mirrors `typst-cli`'s `--input` parser, which is private to that crate.
fn parse_sys_input_pair(raw: &str) -> Result<(String, String), String> {
    let (key, val) = raw
        .split_once('=')
        .ok_or("input must be a key and a value separated by an equal sign")?;
    let key = key.trim().to_owned();
    if key.is_empty() {
        return Err("the key was missing or empty".to_owned());
    }
    let val = val.trim().to_owned();
    Ok((key, val))
}

/// Merges `--input` pairs and an optional `--inputs-file` into one map that
/// feeds `sys.inputs` — never two separate maps. `--input` wins on a key
/// collision (see its help text above).
fn build_inputs(
    input_pairs: &[(String, String)],
    inputs_file: Option<&Path>,
) -> SourceResult<Dict> {
    let mut pairs = Vec::new();
    if let Some(path) = inputs_file {
        pairs.extend(load_inputs_file(path)?);
    }
    pairs.extend(input_pairs.iter().cloned());
    Ok(pairs
        .into_iter()
        .map(|(k, v)| (k.as_str().into(), v.as_str().into_value()))
        .collect())
}

/// Loads `--inputs-file`: a JSON object whose values are all strings.
fn load_inputs_file(path: &Path) -> SourceResult<Vec<(String, String)>> {
    let text = std::fs::read_to_string(path).map_err(|err| {
        eco_vec![error!(
            Span::detached(),
            "failed to read inputs file {}: {err}",
            path.display()
        )]
    })?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|err| {
        eco_vec![error!(
            Span::detached(),
            "failed to parse inputs file {} as JSON: {err}",
            path.display()
        )]
    })?;
    let object = value.as_object().ok_or_else(|| {
        eco_vec![error!(
            Span::detached(),
            "inputs file {} must contain a JSON object at its top level, found {}",
            path.display(),
            json_kind(&value)
        )]
    })?;
    let mut pairs = Vec::with_capacity(object.len());
    for (key, val) in object {
        let Some(s) = val.as_str() else {
            return Err(eco_vec![error!(
                Span::detached(),
                "inputs file {}: value for key {key:?} must be a string, found {}",
                path.display(),
                json_kind(val)
            )]);
        };
        pairs.push((key.clone(), s.to_string()));
    }
    Ok(pairs)
}

fn json_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "a boolean",
        serde_json::Value::Number(_) => "a number",
        serde_json::Value::String(_) => "a string",
        serde_json::Value::Array(_) => "an array",
        serde_json::Value::Object(_) => "an object",
    }
}
