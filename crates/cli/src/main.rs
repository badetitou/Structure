use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::time::Instant;
use structure_core::model::{Cell, Impact, Matrix, Progress};
use structure_core::pipeline::{self, Analysis, Options};

#[derive(Parser)]
#[command(name = "structure", about = "Structure: call-graph matrix and impact")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Analyze a project and emit the matrix as JSON.
    Analyze {
        path: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Re-run the SCIP indexer even if index.scip exists.
        #[arg(long)]
        reindex: bool,
        /// Tree-sitter only.
        #[arg(long)]
        no_scip: bool,
    },
    /// Compute the impact of a diff.
    Impact {
        path: PathBuf,
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        head: Option<String>,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        no_scip: bool,
    },
}

fn run_analysis(path: &Path, reindex: bool, no_scip: bool) -> Result<Analysis> {
    let opts = Options {
        reindex,
        use_scip: !no_scip,
    };
    let t = Instant::now();
    // (step name, start) of the step in progress; its duration is printed
    // when the next step starts.
    let current = std::cell::RefCell::new(None::<(String, Instant)>);
    let analysis = pipeline::analyze(path, &opts, &|p: Progress| {
        let now = Instant::now();
        if let Some((step, start)) = current.replace(Some((p.step.clone(), now))) {
            eprintln!("  {step}: {} ms", now.duration_since(start).as_millis());
        }
        eprintln!("[{:>3}%] {}", p.pct, p.step);
    })?;
    let m = &analysis.matrix;
    eprintln!(
        "language: {:?}, files: {}, symbols: {}, edges: {}, total: {} ms",
        m.language,
        m.stats.files,
        m.stats.symbols,
        m.stats.edges,
        t.elapsed().as_millis()
    );
    match &analysis.link_stats {
        Some(s) => eprintln!("link: {s:?}"),
        None => eprintln!("link: none (tree-sitter only)"),
    }
    for w in &analysis.warnings {
        eprintln!("warning: {w}");
    }
    Ok(analysis)
}

fn print_table(m: &Matrix) {
    let w = 12;
    eprint!("{:<w$}", "");
    for d in &m.domains {
        eprint!(" {:>w$}", d.chars().take(w).collect::<String>());
    }
    eprintln!();
    for l in &m.layers {
        eprint!("{:<w$}", l.as_str());
        for d in &m.domains {
            let id = Cell::id_for(*l, d);
            let n = m.cells.iter().find(|c| c.id == id).map_or(0, |c| c.size);
            if n == 0 {
                eprint!(" {:>w$}", ".");
            } else {
                eprint!(" {n:>w$}");
            }
        }
        eprintln!();
    }
}

fn write_json<T: serde::Serialize>(v: &T, out: &Option<PathBuf>) -> Result<()> {
    let s = serde_json::to_string_pretty(v)?;
    match out {
        Some(p) => std::fs::write(p, s)?,
        None => println!("{s}"),
    }
    Ok(())
}

fn print_impact(i: &Impact) {
    eprintln!("changed files: {}", i.changed_files.len());
    eprintln!("changed cells: {}", i.changed_cells.join(", "));
    for c in &i.impacted_cells {
        eprintln!("impacted cell (distance {}): {}", c.distance, c.id);
    }
    eprintln!("impacted symbols: {}", i.impacted_symbols.len());
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Analyze {
            path,
            output,
            reindex,
            no_scip,
        } => {
            let a = run_analysis(&path, reindex, no_scip)?;
            print_table(&a.matrix);
            write_json(&a.matrix, &output)
        }
        Command::Impact {
            path,
            base,
            head,
            output,
            no_scip,
        } => {
            let a = run_analysis(&path, false, no_scip)?;
            let i = pipeline::impact(&path, &a, base.as_deref(), head.as_deref())?;
            print_impact(&i);
            write_json(&i, &output)
        }
    }
}
