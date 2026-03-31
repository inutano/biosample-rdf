use clap::{Parser, Subcommand};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;

use md5::Digest;

use biosample_rdf::chunk::ChunkWriter;
use biosample_rdf::parser::BioSampleParser;
use biosample_rdf::progress::Progress;

#[derive(Parser)]
#[command(name = "biosample-rdf")]
#[command(about = "Convert NCBI BioSample XML dumps to RDF")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert BioSample XML to RDF (TTL, JSON-LD, N-Triples)
    Convert {
        /// Path to biosample_set.xml(.gz) input file
        #[arg(short, long)]
        input: PathBuf,

        /// Output directory for chunked RDF files
        #[arg(short, long, default_value = "./output")]
        output_dir: PathBuf,

        /// Number of records per output chunk
        #[arg(short, long, default_value_t = 100_000)]
        chunk_size: usize,
    },

    /// Resume a previously interrupted conversion
    Resume {
        /// Output directory containing progress.json
        #[arg(short, long, default_value = "./output")]
        output_dir: PathBuf,
    },

    /// Validate output RDF files
    Validate {
        /// Path to file or directory to validate
        path: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Convert {
            input,
            output_dir,
            chunk_size,
        } => run_convert(&input, &output_dir, chunk_size),
        Commands::Resume { output_dir } => {
            println!("Resuming from {:?}", output_dir);
            todo!("resume not yet implemented")
        }
        Commands::Validate { path } => {
            use biosample_rdf::validate;
            let results = if path.is_dir() {
                validate::validate_directory(&path)
            } else if path.extension().map(|e| e == "ttl").unwrap_or(false) {
                vec![validate::validate_turtle(&path)]
            } else if path.extension().map(|e| e == "nt").unwrap_or(false) {
                vec![validate::validate_ntriples(&path)]
            } else {
                eprintln!("Unsupported file type: {:?}", path);
                std::process::exit(1);
            };
            let mut total_errors = 0;
            let mut total_records = 0;
            for result in &results {
                total_records += result.record_count;
                if result.errors.is_empty() {
                    eprintln!("  OK: {} ({} records)", result.file, result.record_count);
                } else {
                    for err in &result.errors {
                        eprintln!("  ERROR: {} - {}", result.file, err);
                    }
                    total_errors += result.errors.len();
                }
            }
            eprintln!("Validation complete: {} files, {} records, {} errors", results.len(), total_records, total_errors);
            if total_errors > 0 {
                std::process::exit(1);
            }
            Ok(())
        }
    }
}

fn run_convert(input: &PathBuf, output_dir: &PathBuf, chunk_size: usize) -> anyhow::Result<()> {
    // 1. Create output directory
    fs::create_dir_all(output_dir)?;

    // 2. Open error log
    let error_log_path = output_dir.join("errors.log");
    let error_log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&error_log_path)?;
    let mut error_log = BufWriter::new(error_log_file);

    // 3. Compute MD5 of input file
    let input_str = input.to_string_lossy().to_string();
    let source_size = fs::metadata(input)?.len();
    let source_md5 = compute_md5(input)?;

    eprintln!(
        "Converting {:?} -> {:?} (chunk size: {})",
        input, output_dir, chunk_size
    );
    eprintln!("  File size: {} bytes, MD5: {}", source_size, source_md5);

    // 4. Create Progress and ChunkWriter
    let progress = Progress::new(&input_str, source_size, &source_md5);
    let mut chunk_writer = ChunkWriter::new(output_dir, chunk_size, progress)?;

    // 5. Open input — detect .gz extension
    let input_path_str = input.to_string_lossy();
    let is_gzipped = input_path_str.ends_with(".gz");

    // 6. Create BioSampleParser; 7. Loop over records
    let mut records_processed: u64 = 0;
    let mut records_skipped: u64 = 0;

    if is_gzipped {
        let file = File::open(input)?;
        let gz = flate2::read::GzDecoder::new(file);
        let reader = BufReader::new(gz);
        let mut parser = BioSampleParser::new(reader);
        process_records(
            &mut parser,
            &mut chunk_writer,
            &mut error_log,
            &mut records_processed,
            &mut records_skipped,
        )?;
    } else {
        let file = File::open(input)?;
        let reader = BufReader::new(file);
        let mut parser = BioSampleParser::new(reader);
        process_records(
            &mut parser,
            &mut chunk_writer,
            &mut error_log,
            &mut records_processed,
            &mut records_skipped,
        )?;
    }

    // 9. Finish
    chunk_writer.finish()?;

    // 10. Print summary
    eprintln!("\nConversion complete:");
    eprintln!("  Records processed: {}", records_processed);
    eprintln!("  Records skipped:   {}", records_skipped);
    eprintln!("  Output:            {:?}", output_dir);

    Ok(())
}

fn process_records<R: std::io::BufRead, W: Write>(
    parser: &mut BioSampleParser<R>,
    chunk_writer: &mut ChunkWriter,
    error_log: &mut W,
    records_processed: &mut u64,
    records_skipped: &mut u64,
) -> anyhow::Result<()> {
    loop {
        match parser.next_record() {
            Ok(Some(record)) => {
                chunk_writer.add_record(record)?;
                *records_processed += 1;
                // 8. Print progress every 100k records
                if (*records_processed).is_multiple_of(100_000) {
                    eprintln!("  Progress: {} records processed", records_processed);
                }
            }
            Ok(None) => break,
            Err(e) => {
                writeln!(error_log, "{}", e)?;
                chunk_writer.record_skip();
                *records_skipped += 1;
            }
        }
    }
    Ok(())
}

fn compute_md5(path: &PathBuf) -> anyhow::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = md5::Md5::new();
    std::io::copy(&mut file, &mut hasher)?;
    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}
