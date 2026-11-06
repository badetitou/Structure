use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::time::Instant;
use structure_core::metrics::{Metrics, peak_rss_bytes};
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

fn run_analysis(
    path: &Path,
    reindex: bool,
    reindex_if_stale: bool,
    no_scip: bool,
) -> Result<Analysis> {
    let opts = Options {
        reindex,
        reindex_if_stale,
        use_scip: !no_scip,
    };
    let t = Instant::now();
    let analysis = pipeline::analyze(path, &opts, &|p: Progress| {
        if p.detail.is_some() {
            // Live indexer output: not a step change.
            return;
        }
        eprintln!("[{:>3}%] {}", p.pct, p.step);
    })?;
    let m = &analysis.matrix;
    eprintln!(
        "language: {:?}, files: {}, symbols: {}, edges: {}",
        m.language, m.stats.files, m.stats.symbols, m.stats.edges
    );
    match &analysis.link_stats {
        Some(s) => eprintln!("link: {s:?}"),
        None => eprintln!("link: none (tree-sitter only)"),
    }
    for w in &analysis.warnings {
        eprintln!("warning: {w}");
    }
    print_metrics(&analysis.metrics);
    eprintln!("total: {} ms", t.elapsed().as_millis());
    Ok(analysis)
}

fn print_metrics(m: &Metrics) {
    eprintln!("phases:");
    for p in &m.phases {
        let note = p
            .note
            .as_deref()
            .map_or(String::new(), |n| format!("  ({n})"));
        eprintln!("  {:<10} {:>8} ms{note}", p.name, p.ms);
    }
    eprintln!(
        "scip: {} documents, {} occurrences",
        m.scip_documents, m.scip_occurrences
    );
    let fmt = |e: &std::collections::BTreeMap<&str, usize>| {
        let total: usize = e.values().sum();
        let kinds: Vec<String> = e.iter().map(|(k, n)| format!("{k} {n}")).collect();
        format!("{total} ({})", kinds.join(", "))
    };
    eprintln!("edges: {}", fmt(&m.edges_by_kind));
    eprintln!("edges to ext:*: {}", fmt(&m.ext_edges_by_kind));
    match peak_rss_bytes() {
        Some(b) => eprintln!("peak RSS: {:.1} MiB", b as f64 / (1024.0 * 1024.0)),
        None => eprintln!("peak RSS: unavailable"),
    }
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

/// `shown`: warnings already printed by the analysis step.
fn print_impact(i: &Impact, shown: &[String]) {
    for w in i.warnings.iter().filter(|w| !shown.contains(w)) {
        eprintln!("warning: {w}");
    }
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
            let a = run_analysis(&path, reindex, true, no_scip)?;
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
            // A stale index is fine for impact (symbol ranges come from tree-sitter).
            let a = run_analysis(&path, false, false, no_scip)?;
            let t = Instant::now();
            let i = pipeline::impact(&path, &a, base.as_deref(), head.as_deref())?;
            eprintln!("impact: {} ms", t.elapsed().as_millis());
            print_impact(&i, &a.warnings);
            write_json(&i, &output)
        }
    }
}
