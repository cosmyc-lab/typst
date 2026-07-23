use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use ecow::eco_vec;
use typst::compile;
use typst::diag::{SourceResult, error};
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
        Commands::Compile { input, output } => compile_file(&input, &output),
    }
}

fn compile_file(input: &std::path::Path, output: &std::path::Path) -> SourceResult<()> {
    let world = world::CndWorld::new(input).map_err(|err| {
        eco_vec![error!(
            Span::detached(),
            "failed to initialize world: {err}"
        )]
    })?;

    let warned = compile::<CndDocument>(&world);
    for warning in &warned.warnings {
        eprintln!("warning: {warning:?}");
    }

    let document = warned.output?;
    let cnd = cnd_from_document(
        &document,
        world::source_info(&world),
        world::built_at_now(),
    );
    let json = cnd_to_json(&cnd)?;
    std::fs::write(output, json).map_err(|err| {
        eco_vec![error!(
            Span::detached(),
            "failed to write output file: {err}"
        )]
    })?;

    Ok(())
}
