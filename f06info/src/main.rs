//! Dumps information on an F06 file, such as its blocks, etc.

#![allow(clippy::needless_return)] // i'll never forgive rust for this
#![allow(dead_code)] // temporary

use std::collections::BTreeSet;
use std::io::{self, BufReader};
use std::path::PathBuf;

use clap::Parser;
use f06::prelude::*;
use log::{LevelFilter, info, error};

#[derive(Parser)]
#[command(author, version)]
struct Cli {
  /// Disable block merging.
  #[arg(short = 'M', long)]
  no_merge: bool,
  /// Output extra/debug info while parsing.
  #[arg(short, long)]
  verbose: bool,
  /// File path (set to "-" to read from standard input).
  file: PathBuf
}

const INDENT: &str = "  ";

fn main() -> io::Result<()> {
  // init cli stuff
  let args = Cli::parse();
  let log_level = if args.verbose {
    LevelFilter::Debug
  } else {
    LevelFilter::Info
  };
  env_logger::builder().filter_level(log_level).init();
  // parse the file
  let mut f06: F06File = if args.file.as_os_str().eq_ignore_ascii_case("-") {
    OnePassParser::parse_bufread(BufReader::new(io::stdin()))?
  } else if args.file.is_file() {
    if let Some(bn) = args.file.file_name() {
      if let Some(sbn) = bn.to_str() {
        info!("Parsing {}...", sbn);
      }
    } else {
      info!("Parsing...");
    }
    OnePassParser::parse_file(&args.file)?
  } else {
    error!("Provided path either does not exist or is not a file!");
    std::process::exit(1);
  };
  // print block & merge info
  info!("Done parsing; decoded {} blocks.", f06.blocks.len());
  // print warnings
  if f06.warnings.is_empty() {
    info!("No warnings found.");
  } else {
    info!("The following warnings were found:");
    for (line, text) in f06.warnings.iter() {
      info!("{}- Line {}: {}", INDENT, line, text);
    }
  }
  // print fatals
  if f06.fatal_errors.is_empty() {
    info!("No fatal errors found.");
  } else {
    info!("The following fatal errors were found:");
    for (line, text) in f06.fatal_errors.iter() {
      info!("{}- Line {}: {}", INDENT, line, text);
    }
  }
  // print merge/block info
  if f06.blocks.is_empty() {
    info!("No supported blocks were found.");
  } else {
    if args.no_merge {
      info!("Merged no blocks, stayed with {}.", f06.blocks.len());
    } else {
      let nmerges = f06.merge_blocks();
      info!("Did {} block merges, now there are {}.", nmerges, f06.blocks.len());
    };
    info!("Supported blocks found:");
    let subcases: BTreeSet<usize> = f06.blocks.iter()
      .map(|b| b.subcase)
      .collect();
    for subcase in subcases {
      info!("{}- Subcase {}:", INDENT, subcase);
      for block in f06.blocks.iter().filter(|b| b.subcase == subcase) {
        info!(
          "{}{}- {}: {} rows, {} columns",
          INDENT,
          INDENT,
          block.block_type,
          block.row_indexes.len(),
          block.col_indexes.len()
        );
      }
    }
  }
  if f06.potential_headers.is_empty() {
    info!("No potential headers for unsupported blocks were found.");
  } else {
    f06.merge_potential_headers();
    info!("Some potential headers for unsupported lines were found:");
    for ph in f06.potential_headers.iter() {
      if ph.span == 1 {
        info!("{}- Line {}: \"{}\"", INDENT, ph.start, ph.text);
      } else {
        info!(
          "{}- Lines {}-{}: \"{}\"",
          INDENT,
          ph.start,
          ph.lines().last().unwrap(),
          ph.text
        );
      }
    }
  }
  return Ok(());
}