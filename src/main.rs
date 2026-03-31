use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
        /// Path to biosample_set.xml.gz input file
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
        Commands::Convert { input, output_dir, chunk_size } => {
            println!("Converting {:?} -> {:?} (chunk size: {})", input, output_dir, chunk_size);
            todo!("convert not yet implemented")
        }
        Commands::Resume { output_dir } => {
            println!("Resuming from {:?}", output_dir);
            todo!("resume not yet implemented")
        }
        Commands::Validate { path } => {
            println!("Validating {:?}", path);
            todo!("validate not yet implemented")
        }
    }
}
