# biosample-rdf Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust CLI that streams NCBI BioSample XML dumps into chunked TTL, JSON-LD, and N-Triples output with crash resilience.

**Architecture:** Single-pass streaming pipeline using `quick-xml` pull parser over gzip-decompressed input. Records are buffered into 100k-record chunks, serialized to three RDF formats, and flushed to disk with progress tracking for crash recovery.

**Tech Stack:** Rust, quick-xml, flate2, clap, serde/serde_json

**Spec:** `docs/superpowers/specs/2026-03-31-biosample-rdf-design.md`

---

## Prerequisites

Rust toolchain is not currently installed on this machine. The first task includes installing it.

## File Map

| File | Responsibility |
|------|---------------|
| `Cargo.toml` | Project manifest and dependencies |
| `src/main.rs` | CLI entry point with clap subcommands |
| `src/error.rs` | Error types for the pipeline |
| `src/model.rs` | `BioSampleRecord` and `Attribute` structs |
| `src/parser.rs` | Streaming XML pull parser |
| `src/serializer/mod.rs` | Serializer trait and module exports |
| `src/serializer/turtle.rs` | Turtle serializer |
| `src/serializer/ntriples.rs` | N-Triples serializer |
| `src/serializer/jsonld.rs` | JSON-LD serializer |
| `src/chunk.rs` | Chunk buffer, flush logic, output directory management |
| `src/progress.rs` | Progress tracking and resume |
| `src/validate.rs` | Validation subcommand |
| `tests/fixtures/biosample_set.xml.10` | 10-record sample XML from old repo |
| `tests/fixtures/quotes.xml` | Edge case: HTML entities in attribute names |
| `tests/fixtures/with_blank_attrs.xml` | Edge case: empty attribute values |
| `tests/fixtures/malformed_record.xml` | Hand-crafted invalid XML |
| `tests/parser_test.rs` | Parser integration tests |
| `tests/golden_test.rs` | End-to-end golden file comparison |
| `Dockerfile` | Multi-stage build container |
| `scripts/daily_update.sh` | Cron wrapper script |

---

### Task 1: Project Scaffolding & Rust Setup

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/error.rs`

- [ ] **Step 1: Install Rust toolchain**

Run:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

Expected: `rustc --version` prints a version like `rustc 1.x.x`.

- [ ] **Step 2: Initialize the Cargo project**

Run:
```bash
cd ~/repos/biosample-rdf
cargo init --name biosample-rdf
```

Expected: Creates `Cargo.toml` and `src/main.rs`.

- [ ] **Step 3: Add dependencies to Cargo.toml**

Replace `Cargo.toml` with:

```toml
[package]
name = "biosample-rdf"
version = "0.1.0"
edition = "2021"
description = "Convert NCBI BioSample XML dumps to RDF (Turtle, JSON-LD, N-Triples)"

[dependencies]
quick-xml = "0.37"
flate2 = "1"
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
md-5 = "0.10"
anyhow = "1"
thiserror = "2"
chrono = { version = "0.4", features = ["serde"] }
percent-encoding = "2"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 4: Create src/error.rs**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BsError {
    #[error("XML parse error at byte {offset}: {message}")]
    XmlParse { offset: u64, message: String },

    #[error("Record missing accession at byte {offset}")]
    MissingAccession { offset: u64 },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
```

- [ ] **Step 5: Create initial src/main.rs with clap skeleton**

```rust
mod error;

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
```

- [ ] **Step 6: Verify it compiles and CLI help works**

Run:
```bash
cd ~/repos/biosample-rdf && cargo build
```

Expected: Compiles successfully.

Run:
```bash
cargo run -- --help
```

Expected: Prints help with `convert`, `resume`, `validate` subcommands.

- [ ] **Step 7: Commit**

```bash
cd ~/repos/biosample-rdf
git add Cargo.toml Cargo.lock src/main.rs src/error.rs
git commit -m "feat: scaffold project with clap CLI and error types"
```

---

### Task 2: Data Model

**Files:**
- Create: `src/model.rs`
- Modify: `src/main.rs` (add `mod model;`)

- [ ] **Step 1: Write model tests**

Add to `src/model.rs`:

```rust
/// A parsed BioSample record.
#[derive(Debug, Clone, PartialEq)]
pub struct BioSampleRecord {
    pub accession: String,
    pub submission_date: Option<String>,
    pub last_update: Option<String>,
    pub publication_date: Option<String>,
    pub title: Option<String>,
    pub attributes: Vec<Attribute>,
}

/// A single attribute key-value pair from a BioSample record.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub attribute_name: String,
    pub harmonized_name: Option<String>,
    pub display_name: Option<String>,
    pub value: Option<String>,
}

impl BioSampleRecord {
    /// Returns the IRI for this record: http://identifiers.org/biosample/{accession}
    pub fn iri(&self) -> String {
        format!("http://identifiers.org/biosample/{}", self.accession)
    }
}

impl Attribute {
    /// Returns the preferred name for this attribute (harmonized_name > attribute_name).
    pub fn preferred_name(&self) -> &str {
        self.harmonized_name.as_deref().unwrap_or(&self.attribute_name)
    }

    /// Returns the property IRI fragment for this attribute, URL-encoded.
    pub fn property_iri(&self, accession: &str) -> String {
        let name = self.preferred_name();
        let encoded = percent_encoding::utf8_percent_encode(
            name,
            percent_encoding::NON_ALPHANUMERIC,
        );
        format!("http://ddbj.nig.ac.jp/biosample/{}#{}", accession, encoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_iri() {
        let rec = BioSampleRecord {
            accession: "SAMD00000345".to_string(),
            submission_date: None,
            last_update: None,
            publication_date: None,
            title: None,
            attributes: vec![],
        };
        assert_eq!(rec.iri(), "http://identifiers.org/biosample/SAMD00000345");
    }

    #[test]
    fn test_attribute_preferred_name_harmonized() {
        let attr = Attribute {
            attribute_name: "geo_loc_name".to_string(),
            harmonized_name: Some("geo_loc_name".to_string()),
            display_name: Some("geographic location".to_string()),
            value: Some("Japan".to_string()),
        };
        assert_eq!(attr.preferred_name(), "geo_loc_name");
    }

    #[test]
    fn test_attribute_preferred_name_fallback() {
        let attr = Attribute {
            attribute_name: "finishing strategy (depth of coverage)".to_string(),
            harmonized_name: None,
            display_name: None,
            value: Some("Level 3".to_string()),
        };
        assert_eq!(attr.preferred_name(), "finishing strategy (depth of coverage)");
    }

    #[test]
    fn test_attribute_property_iri() {
        let attr = Attribute {
            attribute_name: "organism".to_string(),
            harmonized_name: Some("organism".to_string()),
            display_name: None,
            value: Some("Homo sapiens".to_string()),
        };
        assert_eq!(
            attr.property_iri("SAMD00000345"),
            "http://ddbj.nig.ac.jp/biosample/SAMD00000345#organism"
        );
    }

    #[test]
    fn test_attribute_property_iri_url_encodes_spaces() {
        let attr = Attribute {
            attribute_name: "sample name".to_string(),
            harmonized_name: Some("sample_name".to_string()),
            display_name: None,
            value: Some("test".to_string()),
        };
        assert_eq!(
            attr.property_iri("SAMN00000002"),
            "http://ddbj.nig.ac.jp/biosample/SAMN00000002#sample%5Fname"
        );
    }

    #[test]
    fn test_attribute_with_no_value() {
        let attr = Attribute {
            attribute_name: "exp_ammonium".to_string(),
            harmonized_name: None,
            display_name: None,
            value: None,
        };
        assert_eq!(attr.preferred_name(), "exp_ammonium");
        assert!(attr.value.is_none());
    }
}
```

- [ ] **Step 2: Add mod declaration to main.rs**

Add `mod model;` after `mod error;` in `src/main.rs`:

```rust
mod error;
mod model;
```

- [ ] **Step 3: Run tests**

Run:
```bash
cd ~/repos/biosample-rdf && cargo test model::tests
```

Expected: All 6 tests pass.

- [ ] **Step 4: Commit**

```bash
cd ~/repos/biosample-rdf
git add src/model.rs src/main.rs
git commit -m "feat: add BioSampleRecord and Attribute data model"
```

---

### Task 3: XML Streaming Parser

**Files:**
- Create: `src/parser.rs`
- Create: `tests/fixtures/biosample_set.xml.10` (copy from old repo)
- Create: `tests/fixtures/quotes.xml`
- Create: `tests/fixtures/with_blank_attrs.xml`
- Create: `tests/fixtures/malformed_record.xml`
- Create: `tests/parser_test.rs`
- Modify: `src/main.rs` (add `mod parser;`)

- [ ] **Step 1: Copy test fixtures from the old repo**

Download the fixture files:

```bash
cd ~/repos/biosample-rdf
mkdir -p tests/fixtures
gh api repos/inutano/biosample_jsonld/contents/example/biosample_set.xml.10 --jq '.content' | base64 -d > tests/fixtures/biosample_set.xml.10
gh api repos/inutano/biosample_jsonld/contents/example/quotes.xml --jq '.content' | base64 -d > tests/fixtures/quotes.xml
gh api repos/inutano/biosample_jsonld/contents/example/with_blank_attrs.xml --jq '.content' | base64 -d > tests/fixtures/with_blank_attrs.xml
```

Create `tests/fixtures/malformed_record.xml`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<BioSampleSet>
<BioSample submission_date="2020-01-01T00:00:00.000" last_update="2020-01-01T00:00:00.000" publication_date="2020-01-01T00:00:00.000" access="public" id="999" accession="SAMN99999999">
  <Ids>
    <Id db="BioSample" is_primary="1">SAMN99999999</Id>
  </Ids>
  <Description>
    <Title>Good record</Title>
  </Description>
  <Attributes>
    <Attribute attribute_name="organism" harmonized_name="organism">Homo sapiens</Attribute>
  </Attributes>
</BioSample>
<BioSample submission_date="2020-01-01T00:00:00.000" access="public" id="998">
  <Ids>
    <Id db="BioSample" is_primary="1">BROKEN</Id>
  </Ids>
  <Description>
    <Title>Record missing accession attribute</Title>
  </Description>
  <Attributes>
    <Attribute attribute_name="organism">test</Attribute>
  </Attributes>
</BioSample>
<BioSample submission_date="2020-01-01T00:00:00.000" last_update="2020-01-01T00:00:00.000" access="public" id="997" accession="SAMN99999997">
  <Ids>
    <Id db="BioSample" is_primary="1">SAMN99999997</Id>
  </Ids>
  <Description>
    <Title>Record after bad one</Title>
  </Description>
  <Attributes>
    <Attribute attribute_name="strain" harmonized_name="strain">ABC</Attribute>
  </Attributes>
</BioSample>
</BioSampleSet>
```

- [ ] **Step 2: Write the parser**

Create `src/parser.rs`:

```rust
use crate::error::BsError;
use crate::model::{Attribute, BioSampleRecord};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use std::io::BufRead;

/// Streaming parser that yields one BioSampleRecord at a time from XML input.
pub struct BioSampleParser<R: BufRead> {
    reader: Reader<R>,
    buf: Vec<u8>,
}

impl<R: BufRead> BioSampleParser<R> {
    pub fn new(reader: R) -> Self {
        let mut xml_reader = Reader::from_reader(reader);
        xml_reader.config_mut().trim_text(true);
        Self {
            reader: xml_reader,
            buf: Vec::with_capacity(8192),
        }
    }

    /// Returns the next successfully parsed record, or None at EOF.
    /// Parse errors for individual records are returned as Err — the caller
    /// should log them and call next_record() again to continue.
    pub fn next_record(&mut self) -> Result<Option<BioSampleRecord>, BsError> {
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"BioSample" => {
                    return self.parse_biosample(e);
                }
                Ok(Event::Eof) => return Ok(None),
                Ok(_) => continue,
                Err(e) => {
                    return Err(BsError::XmlParse {
                        offset: self.reader.error_position() as u64,
                        message: e.to_string(),
                    });
                }
            }
        }
    }

    fn parse_biosample(
        &mut self,
        start: &BytesStart,
    ) -> Result<Option<BioSampleRecord>, BsError> {
        let offset = self.reader.buffer_position() as u64;

        // Extract attributes from <BioSample> element
        let mut accession: Option<String> = None;
        let mut submission_date: Option<String> = None;
        let mut last_update: Option<String> = None;
        let mut publication_date: Option<String> = None;

        for attr_result in start.attributes() {
            let attr = attr_result.map_err(|e| BsError::XmlParse {
                offset,
                message: format!("attribute error: {}", e),
            })?;
            let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
            let val = attr.unescape_value().map_err(|e| BsError::XmlParse {
                offset,
                message: format!("unescape error: {}", e),
            })?;
            match key {
                "accession" => accession = Some(val.to_string()),
                "submission_date" => submission_date = Some(val.to_string()),
                "last_update" => last_update = Some(val.to_string()),
                "publication_date" => publication_date = Some(val.to_string()),
                _ => {}
            }
        }

        let accession = accession.ok_or_else(|| BsError::MissingAccession { offset })?;

        let mut title: Option<String> = None;
        let mut attributes: Vec<Attribute> = Vec::new();
        let mut in_title = false;
        let mut in_attribute = false;
        let mut current_attr_name = String::new();
        let mut current_harmonized: Option<String> = None;
        let mut current_display: Option<String> = None;
        let mut current_text = String::new();

        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let tag = e.name();
                    match tag.as_ref() {
                        b"Title" => {
                            in_title = true;
                            current_text.clear();
                        }
                        b"Attribute" => {
                            in_attribute = true;
                            current_text.clear();
                            current_attr_name.clear();
                            current_harmonized = None;
                            current_display = None;

                            for attr_result in e.attributes() {
                                if let Ok(attr) = attr_result {
                                    let key =
                                        std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                                    let val = attr
                                        .unescape_value()
                                        .unwrap_or_default();
                                    match key {
                                        "attribute_name" => {
                                            current_attr_name = val.to_string();
                                        }
                                        "harmonized_name" => {
                                            current_harmonized = Some(val.to_string());
                                        }
                                        "display_name" => {
                                            current_display = Some(val.to_string());
                                        }
                                        _ => {}
                                    }
                                }
                            }

                            // Handle self-closing <Attribute .../> (empty value)
                            if matches!(self.buf.as_slice(), _ if e.name().as_ref() == b"Attribute")
                                && matches!(
                                    self.reader.read_event_into(&mut Vec::new()),
                                    _ // peek check not needed for Empty
                                )
                            {
                                // Actually, for Event::Empty we should push immediately
                            }

                            if matches!(
                                &self.buf,
                                _ // check if it was Empty event
                            ) {
                                // We'll handle Empty below
                            }
                        }
                        _ => {}
                    }

                    // If it was an Empty event (self-closing tag), handle end immediately
                    if matches!(self.reader.read_event_into(&mut Vec::new()), _) {
                        // This approach is wrong — let's restructure
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if in_title || in_attribute {
                        let text = e.unescape().unwrap_or_default();
                        current_text.push_str(&text);
                    }
                }
                Ok(Event::End(ref e)) => {
                    match e.name().as_ref() {
                        b"Title" => {
                            if in_title {
                                title = Some(current_text.clone());
                                in_title = false;
                            }
                        }
                        b"Attribute" => {
                            if in_attribute {
                                let value = if current_text.is_empty() {
                                    None
                                } else {
                                    Some(current_text.clone())
                                };
                                attributes.push(Attribute {
                                    attribute_name: current_attr_name.clone(),
                                    harmonized_name: current_harmonized.take(),
                                    display_name: current_display.take(),
                                    value,
                                });
                                in_attribute = false;
                            }
                        }
                        b"BioSample" => break,
                        _ => {}
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(BsError::XmlParse {
                        offset: self.reader.error_position() as u64,
                        message: e.to_string(),
                    });
                }
                _ => {}
            }
        }

        Ok(Some(BioSampleRecord {
            accession,
            submission_date,
            last_update,
            publication_date,
            title,
            attributes,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_xml(xml: &str) -> Vec<Result<BioSampleRecord, BsError>> {
        let mut parser = BioSampleParser::new(xml.as_bytes());
        let mut results = Vec::new();
        loop {
            match parser.next_record() {
                Ok(Some(rec)) => results.push(Ok(rec)),
                Ok(None) => break,
                Err(e) => results.push(Err(e)),
            }
        }
        results
    }

    #[test]
    fn test_parse_single_record() {
        let xml = r#"<?xml version="1.0"?>
<BioSampleSet>
<BioSample submission_date="2020-01-01T00:00:00.000" last_update="2020-06-01T00:00:00.000" publication_date="2020-01-01T00:00:00.000" access="public" id="1" accession="SAMN00000001">
  <Description>
    <Title>Test sample</Title>
  </Description>
  <Attributes>
    <Attribute attribute_name="organism" harmonized_name="organism" display_name="organism">Homo sapiens</Attribute>
    <Attribute attribute_name="strain" harmonized_name="strain">ABC123</Attribute>
  </Attributes>
</BioSample>
</BioSampleSet>"#;

        let results = parse_xml(xml);
        assert_eq!(results.len(), 1);
        let rec = results[0].as_ref().unwrap();
        assert_eq!(rec.accession, "SAMN00000001");
        assert_eq!(rec.title.as_deref(), Some("Test sample"));
        assert_eq!(rec.submission_date.as_deref(), Some("2020-01-01T00:00:00.000"));
        assert_eq!(rec.attributes.len(), 2);
        assert_eq!(rec.attributes[0].attribute_name, "organism");
        assert_eq!(rec.attributes[0].harmonized_name.as_deref(), Some("organism"));
        assert_eq!(rec.attributes[0].value.as_deref(), Some("Homo sapiens"));
        assert_eq!(rec.attributes[1].attribute_name, "strain");
        assert_eq!(rec.attributes[1].value.as_deref(), Some("ABC123"));
    }

    #[test]
    fn test_parse_empty_attribute_value() {
        let xml = r#"<?xml version="1.0"?>
<BioSampleSet>
<BioSample access="public" id="1" accession="SAMN00000001">
  <Description><Title>Test</Title></Description>
  <Attributes>
    <Attribute attribute_name="exp_ammonium"></Attribute>
  </Attributes>
</BioSample>
</BioSampleSet>"#;

        let results = parse_xml(xml);
        let rec = results[0].as_ref().unwrap();
        assert_eq!(rec.attributes.len(), 1);
        assert!(rec.attributes[0].value.is_none());
    }

    #[test]
    fn test_missing_accession_returns_error() {
        let xml = r#"<?xml version="1.0"?>
<BioSampleSet>
<BioSample access="public" id="1">
  <Description><Title>No accession</Title></Description>
  <Attributes></Attributes>
</BioSample>
</BioSampleSet>"#;

        let results = parse_xml(xml);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_err());
    }

    #[test]
    fn test_record_without_title() {
        let xml = r#"<?xml version="1.0"?>
<BioSampleSet>
<BioSample access="public" id="1" accession="SAMN00000001">
  <Attributes>
    <Attribute attribute_name="organism">test</Attribute>
  </Attributes>
</BioSample>
</BioSampleSet>"#;

        let results = parse_xml(xml);
        let rec = results[0].as_ref().unwrap();
        assert!(rec.title.is_none());
    }
}
```

Note: The initial parser implementation above has some structural issues with the Empty event handling. The implementation step below provides the clean version.

- [ ] **Step 3: Write the clean parser implementation**

Replace the full content of `src/parser.rs` with:

```rust
use crate::error::BsError;
use crate::model::{Attribute, BioSampleRecord};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::io::BufRead;

/// Streaming parser that yields one BioSampleRecord at a time from XML input.
pub struct BioSampleParser<R: BufRead> {
    reader: Reader<R>,
    buf: Vec<u8>,
}

impl<R: BufRead> BioSampleParser<R> {
    pub fn new(reader: R) -> Self {
        let mut xml_reader = Reader::from_reader(reader);
        xml_reader.config_mut().trim_text(true);
        Self {
            reader: xml_reader,
            buf: Vec::with_capacity(8192),
        }
    }

    /// Returns the next successfully parsed record, or None at EOF.
    /// Parse errors for individual records are returned as Err — the caller
    /// should log them and call next_record() again to continue.
    pub fn next_record(&mut self) -> Result<Option<BioSampleRecord>, BsError> {
        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(ref e)) if e.name().as_ref() == b"BioSample" => {
                    let offset = self.reader.buffer_position() as u64;

                    // Extract attributes from <BioSample> element
                    let mut accession: Option<String> = None;
                    let mut submission_date: Option<String> = None;
                    let mut last_update: Option<String> = None;
                    let mut publication_date: Option<String> = None;

                    for attr_result in e.attributes() {
                        let attr = attr_result.map_err(|e| BsError::XmlParse {
                            offset,
                            message: format!("attribute error: {}", e),
                        })?;
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = attr.unescape_value().map_err(|e| BsError::XmlParse {
                            offset,
                            message: format!("unescape error: {}", e),
                        })?;
                        match key {
                            "accession" => accession = Some(val.to_string()),
                            "submission_date" => submission_date = Some(val.to_string()),
                            "last_update" => last_update = Some(val.to_string()),
                            "publication_date" => publication_date = Some(val.to_string()),
                            _ => {}
                        }
                    }

                    let accession =
                        accession.ok_or_else(|| BsError::MissingAccession { offset })?;

                    return self.parse_biosample_body(
                        accession,
                        submission_date,
                        last_update,
                        publication_date,
                    );
                }
                Ok(Event::Eof) => return Ok(None),
                Ok(_) => continue,
                Err(e) => {
                    return Err(BsError::XmlParse {
                        offset: self.reader.error_position() as u64,
                        message: e.to_string(),
                    });
                }
            }
        }
    }

    fn parse_biosample_body(
        &mut self,
        accession: String,
        submission_date: Option<String>,
        last_update: Option<String>,
        publication_date: Option<String>,
    ) -> Result<Option<BioSampleRecord>, BsError> {
        let mut title: Option<String> = None;
        let mut attributes: Vec<Attribute> = Vec::new();
        let mut in_title = false;
        let mut in_attribute = false;
        let mut current_attr_name = String::new();
        let mut current_harmonized: Option<String> = None;
        let mut current_display: Option<String> = None;
        let mut current_text = String::new();
        let mut depth: u32 = 1; // We're inside <BioSample>

        loop {
            self.buf.clear();
            match self.reader.read_event_into(&mut self.buf) {
                Ok(Event::Start(ref e)) => {
                    depth += 1;
                    match e.name().as_ref() {
                        b"Title" => {
                            in_title = true;
                            current_text.clear();
                        }
                        b"Attribute" => {
                            in_attribute = true;
                            current_text.clear();
                            current_attr_name.clear();
                            current_harmonized = None;
                            current_display = None;

                            for attr_result in e.attributes() {
                                if let Ok(attr) = attr_result {
                                    let key =
                                        std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                                    let val = attr.unescape_value().unwrap_or_default();
                                    match key {
                                        "attribute_name" => {
                                            current_attr_name = val.to_string();
                                        }
                                        "harmonized_name" => {
                                            current_harmonized = Some(val.to_string());
                                        }
                                        "display_name" => {
                                            current_display = Some(val.to_string());
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    // Self-closing tags like <Attribute .../> or <Organism .../>
                    if e.name().as_ref() == b"Attribute" {
                        let mut attr_name = String::new();
                        let mut harmonized = None;
                        let mut display = None;

                        for attr_result in e.attributes() {
                            if let Ok(attr) = attr_result {
                                let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                                let val = attr.unescape_value().unwrap_or_default();
                                match key {
                                    "attribute_name" => attr_name = val.to_string(),
                                    "harmonized_name" => harmonized = Some(val.to_string()),
                                    "display_name" => display = Some(val.to_string()),
                                    _ => {}
                                }
                            }
                        }

                        attributes.push(Attribute {
                            attribute_name: attr_name,
                            harmonized_name: harmonized,
                            display_name: display,
                            value: None,
                        });
                    }
                }
                Ok(Event::Text(ref e)) => {
                    if in_title || in_attribute {
                        let text = e.unescape().unwrap_or_default();
                        current_text.push_str(&text);
                    }
                }
                Ok(Event::End(ref e)) => {
                    depth -= 1;
                    match e.name().as_ref() {
                        b"Title" if in_title => {
                            title = Some(current_text.clone());
                            in_title = false;
                        }
                        b"Attribute" if in_attribute => {
                            let value = if current_text.is_empty() {
                                None
                            } else {
                                Some(current_text.clone())
                            };
                            attributes.push(Attribute {
                                attribute_name: current_attr_name.clone(),
                                harmonized_name: current_harmonized.take(),
                                display_name: current_display.take(),
                                value,
                            });
                            in_attribute = false;
                        }
                        b"BioSample" if depth == 0 => break,
                        _ => {}
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => {
                    // Skip to end of this BioSample on error
                    break;
                }
                _ => {}
            }
        }

        Ok(Some(BioSampleRecord {
            accession,
            submission_date,
            last_update,
            publication_date,
            title,
            attributes,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_all(xml: &str) -> Vec<Result<BioSampleRecord, BsError>> {
        let mut parser = BioSampleParser::new(xml.as_bytes());
        let mut results = Vec::new();
        loop {
            match parser.next_record() {
                Ok(Some(rec)) => results.push(Ok(rec)),
                Ok(None) => break,
                Err(e) => results.push(Err(e)),
            }
        }
        results
    }

    #[test]
    fn test_parse_single_record() {
        let xml = r#"<?xml version="1.0"?>
<BioSampleSet>
<BioSample submission_date="2020-01-01T00:00:00.000" last_update="2020-06-01T00:00:00.000" publication_date="2020-01-01T00:00:00.000" access="public" id="1" accession="SAMN00000001">
  <Description>
    <Title>Test sample</Title>
  </Description>
  <Attributes>
    <Attribute attribute_name="organism" harmonized_name="organism" display_name="organism">Homo sapiens</Attribute>
    <Attribute attribute_name="strain" harmonized_name="strain">ABC123</Attribute>
  </Attributes>
</BioSample>
</BioSampleSet>"#;

        let results = parse_all(xml);
        assert_eq!(results.len(), 1);
        let rec = results[0].as_ref().unwrap();
        assert_eq!(rec.accession, "SAMN00000001");
        assert_eq!(rec.title.as_deref(), Some("Test sample"));
        assert_eq!(rec.submission_date.as_deref(), Some("2020-01-01T00:00:00.000"));
        assert_eq!(rec.last_update.as_deref(), Some("2020-06-01T00:00:00.000"));
        assert_eq!(rec.attributes.len(), 2);
        assert_eq!(rec.attributes[0].attribute_name, "organism");
        assert_eq!(rec.attributes[0].harmonized_name.as_deref(), Some("organism"));
        assert_eq!(rec.attributes[0].display_name.as_deref(), Some("organism"));
        assert_eq!(rec.attributes[0].value.as_deref(), Some("Homo sapiens"));
    }

    #[test]
    fn test_parse_empty_attribute_value() {
        let xml = r#"<?xml version="1.0"?>
<BioSampleSet>
<BioSample access="public" id="1" accession="SAMN00000001">
  <Description><Title>Test</Title></Description>
  <Attributes>
    <Attribute attribute_name="exp_ammonium"></Attribute>
  </Attributes>
</BioSample>
</BioSampleSet>"#;

        let results = parse_all(xml);
        let rec = results[0].as_ref().unwrap();
        assert_eq!(rec.attributes.len(), 1);
        assert!(rec.attributes[0].value.is_none());
    }

    #[test]
    fn test_missing_accession_returns_error() {
        let xml = r#"<?xml version="1.0"?>
<BioSampleSet>
<BioSample access="public" id="1">
  <Description><Title>No accession</Title></Description>
</BioSample>
</BioSampleSet>"#;

        let results = parse_all(xml);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_err());
    }

    #[test]
    fn test_continues_after_bad_record() {
        let xml = r#"<?xml version="1.0"?>
<BioSampleSet>
<BioSample access="public" id="1">
  <Description><Title>No accession</Title></Description>
</BioSample>
<BioSample access="public" id="2" accession="SAMN00000002">
  <Description><Title>Good record</Title></Description>
  <Attributes>
    <Attribute attribute_name="organism">test</Attribute>
  </Attributes>
</BioSample>
</BioSampleSet>"#;

        let results = parse_all(xml);
        assert_eq!(results.len(), 2);
        assert!(results[0].is_err());
        let rec = results[1].as_ref().unwrap();
        assert_eq!(rec.accession, "SAMN00000002");
    }

    #[test]
    fn test_record_without_title() {
        let xml = r#"<?xml version="1.0"?>
<BioSampleSet>
<BioSample access="public" id="1" accession="SAMN00000001">
  <Attributes>
    <Attribute attribute_name="organism">test</Attribute>
  </Attributes>
</BioSample>
</BioSampleSet>"#;

        let results = parse_all(xml);
        let rec = results[0].as_ref().unwrap();
        assert!(rec.title.is_none());
        assert_eq!(rec.attributes.len(), 1);
    }

    #[test]
    fn test_html_entity_in_attribute_name() {
        // The quotes.xml fixture has &quot;PUBLIC&quot; as an attribute name
        let xml = r#"<?xml version="1.0"?>
<BioSampleSet>
<BioSample access="public" id="1" accession="SAMN00000001">
  <Description><Title>Test</Title></Description>
  <Attributes>
    <Attribute attribute_name="&quot;PUBLIC&quot;">n</Attribute>
  </Attributes>
</BioSample>
</BioSampleSet>"#;

        let results = parse_all(xml);
        let rec = results[0].as_ref().unwrap();
        assert_eq!(rec.attributes[0].attribute_name, "\"PUBLIC\"");
        assert_eq!(rec.attributes[0].value.as_deref(), Some("n"));
    }

    #[test]
    fn test_multiple_records() {
        let xml = r#"<?xml version="1.0"?>
<BioSampleSet>
<BioSample access="public" id="1" accession="SAMN00000001" submission_date="2020-01-01T00:00:00.000">
  <Description><Title>First</Title></Description>
  <Attributes><Attribute attribute_name="a">1</Attribute></Attributes>
</BioSample>
<BioSample access="public" id="2" accession="SAMN00000002" submission_date="2020-02-01T00:00:00.000">
  <Description><Title>Second</Title></Description>
  <Attributes><Attribute attribute_name="b">2</Attribute></Attributes>
</BioSample>
</BioSampleSet>"#;

        let results = parse_all(xml);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].as_ref().unwrap().accession, "SAMN00000001");
        assert_eq!(results[1].as_ref().unwrap().accession, "SAMN00000002");
    }
}
```

- [ ] **Step 4: Add mod declaration to main.rs**

Add `mod parser;` to `src/main.rs`:

```rust
mod error;
mod model;
mod parser;
```

- [ ] **Step 5: Run tests**

Run:
```bash
cd ~/repos/biosample-rdf && cargo test
```

Expected: All model and parser tests pass.

- [ ] **Step 6: Commit**

```bash
cd ~/repos/biosample-rdf
git add src/parser.rs src/main.rs tests/fixtures/
git commit -m "feat: add streaming XML parser with error recovery"
```

---

### Task 4: Integration test with 10-record fixture

**Files:**
- Create: `tests/parser_test.rs`

- [ ] **Step 1: Write integration test**

Create `tests/parser_test.rs`:

```rust
use std::fs::File;
use std::io::BufReader;

// We need to import from the crate
// Since parser is not pub, we test via the binary or make it pub
// For integration tests, we'll parse the fixture directly

#[test]
fn test_parse_10_record_fixture() {
    let file = File::open("tests/fixtures/biosample_set.xml.10")
        .expect("fixture file not found");
    let reader = BufReader::new(file);

    let mut parser = biosample_rdf::parser::BioSampleParser::new(reader);
    let mut records = Vec::new();
    let mut errors = Vec::new();

    loop {
        match parser.next_record() {
            Ok(Some(rec)) => records.push(rec),
            Ok(None) => break,
            Err(e) => errors.push(e),
        }
    }

    // The fixture has 10 BioSample records
    assert_eq!(records.len(), 10, "Expected 10 records, got {}", records.len());
    assert_eq!(errors.len(), 0, "Expected no errors, got {:?}", errors);

    // Verify first record
    assert_eq!(records[0].accession, "SAMN00000002");
    assert_eq!(records[0].title.as_deref(), Some("Alistipes putredinis DSM 17216"));
    assert!(records[0].attributes.len() > 10);

    // Verify last record has attributes
    assert!(!records[9].attributes.is_empty());
}

#[test]
fn test_parse_quotes_fixture() {
    let file = File::open("tests/fixtures/quotes.xml")
        .expect("fixture file not found");
    let reader = BufReader::new(file);

    let mut parser = biosample_rdf::parser::BioSampleParser::new(reader);
    let mut records = Vec::new();

    loop {
        match parser.next_record() {
            Ok(Some(rec)) => records.push(rec),
            Ok(None) => break,
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].accession, "SAMEA2388127");

    // Find the &quot;PUBLIC&quot; attribute
    let public_attr = records[0]
        .attributes
        .iter()
        .find(|a| a.attribute_name.contains("PUBLIC"))
        .expect("Should find PUBLIC attribute");
    assert_eq!(public_attr.attribute_name, "\"PUBLIC\"");
}

#[test]
fn test_parse_blank_attrs_fixture() {
    let file = File::open("tests/fixtures/with_blank_attrs.xml")
        .expect("fixture file not found");
    let reader = BufReader::new(file);

    let mut parser = biosample_rdf::parser::BioSampleParser::new(reader);
    let mut records = Vec::new();

    loop {
        match parser.next_record() {
            Ok(Some(rec)) => records.push(rec),
            Ok(None) => break,
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].accession, "SAMN02739938");

    // Find empty attributes (exp_ammonium, exp_urea, etc.)
    let empty_attrs: Vec<_> = records[0]
        .attributes
        .iter()
        .filter(|a| a.value.is_none())
        .collect();
    assert!(
        empty_attrs.len() >= 5,
        "Expected at least 5 empty attributes, got {}",
        empty_attrs.len()
    );
}

#[test]
fn test_parse_malformed_fixture() {
    let file = File::open("tests/fixtures/malformed_record.xml")
        .expect("fixture file not found");
    let reader = BufReader::new(file);

    let mut parser = biosample_rdf::parser::BioSampleParser::new(reader);
    let mut records = Vec::new();
    let mut errors = Vec::new();

    loop {
        match parser.next_record() {
            Ok(Some(rec)) => records.push(rec),
            Ok(None) => break,
            Err(e) => errors.push(e),
        }
    }

    // Should get 2 good records and 1 error (missing accession)
    assert_eq!(records.len(), 2, "Expected 2 valid records");
    assert_eq!(errors.len(), 1, "Expected 1 error for missing accession");
    assert_eq!(records[0].accession, "SAMN99999999");
    assert_eq!(records[1].accession, "SAMN99999997");
}
```

- [ ] **Step 2: Make parser module public for integration tests**

In `src/main.rs`, change to use `lib.rs` pattern. Create `src/lib.rs`:

```rust
pub mod error;
pub mod model;
pub mod parser;
```

Update `src/main.rs` — remove the `mod` declarations and use the library:

```rust
use biosample_rdf::error;
use biosample_rdf::model;
use biosample_rdf::parser;

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
```

- [ ] **Step 3: Run all tests**

Run:
```bash
cd ~/repos/biosample-rdf && cargo test
```

Expected: All unit tests and integration tests pass.

- [ ] **Step 4: Commit**

```bash
cd ~/repos/biosample-rdf
git add src/lib.rs src/main.rs tests/parser_test.rs
git commit -m "test: add integration tests for parser with real fixtures"
```

---

### Task 5: Turtle Serializer

**Files:**
- Create: `src/serializer/mod.rs`
- Create: `src/serializer/turtle.rs`
- Modify: `src/lib.rs` (add `pub mod serializer;`)

- [ ] **Step 1: Create serializer module**

Create `src/serializer/mod.rs`:

```rust
pub mod turtle;
pub mod ntriples;
pub mod jsonld;

use crate::model::BioSampleRecord;
use std::io::Write;

/// Shared constants for RDF prefixes.
pub const PREFIX_SCHEMA: &str = "http://schema.org/";
pub const PREFIX_IDORG: &str = "http://identifiers.org/biosample/";
pub const PREFIX_DCT: &str = "http://purl.org/dc/terms/";
pub const PREFIX_DDBJ: &str = "http://ddbj.nig.ac.jp/biosample/";
pub const PREFIX_DDBJONT: &str = "http://ddbj.nig.ac.jp/ontologies/biosample/";
pub const PREFIX_RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
pub const PREFIX_XSD: &str = "https://www.w3.org/2001/XMLSchema#";

/// Trait for serializing BioSample records to an output stream.
pub trait Serializer {
    /// Write any header/prefix declarations needed at the start of the file.
    fn write_header<W: Write>(&self, writer: &mut W) -> std::io::Result<()>;

    /// Write a single record.
    fn write_record<W: Write>(&self, writer: &mut W, record: &BioSampleRecord) -> std::io::Result<()>;

    /// Write any footer needed at the end of the file.
    fn write_footer<W: Write>(&self, writer: &mut W) -> std::io::Result<()>;
}
```

- [ ] **Step 2: Create Turtle serializer with tests**

Create `src/serializer/turtle.rs`:

```rust
use crate::model::BioSampleRecord;
use crate::serializer::Serializer;
use std::io::Write;

pub struct TurtleSerializer;

impl TurtleSerializer {
    pub fn new() -> Self {
        Self
    }

    fn escape_turtle_string(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    }
}

impl Serializer for TurtleSerializer {
    fn write_header<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        writeln!(w, "@prefix : <http://schema.org/> .")?;
        writeln!(w, "@prefix idorg: <http://identifiers.org/biosample/> .")?;
        writeln!(w, "@prefix dct: <http://purl.org/dc/terms/> .")?;
        writeln!(w, "@prefix ddbj: <http://ddbj.nig.ac.jp/biosample/> .")?;
        writeln!(w, "@prefix ddbjont: <http://ddbj.nig.ac.jp/ontologies/biosample/> .")?;
        writeln!(w, "@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .")?;
        writeln!(w, "@prefix xsd: <https://www.w3.org/2001/XMLSchema#> .")?;
        writeln!(w)?;
        Ok(())
    }

    fn write_record<W: Write>(&self, w: &mut W, rec: &BioSampleRecord) -> std::io::Result<()> {
        let acc = &rec.accession;
        let desc = rec.title.as_deref().unwrap_or("");
        let desc_escaped = Self::escape_turtle_string(desc);

        writeln!(w)?;
        writeln!(w, "idorg:{}", acc)?;
        writeln!(w, "  a ddbjont:BioSampleRecord ;")?;
        writeln!(w, "  dct:identifier \"{}\" ;", acc)?;
        writeln!(w, "  dct:description \"{}\" ;", desc_escaped)?;
        writeln!(w, "  rdfs:label \"{}\" ;", desc_escaped)?;

        if let Some(ref d) = rec.submission_date {
            writeln!(w, "  dct:created \"{}\"^^xsd:dateTime ;", d)?;
        }
        if let Some(ref d) = rec.last_update {
            writeln!(w, "  dct:modified \"{}\"^^xsd:dateTime ;", d)?;
        }
        if let Some(ref d) = rec.publication_date {
            writeln!(w, "  dct:issued \"{}\"^^xsd:dateTime ;", d)?;
        }

        let attrs_with_values: Vec<_> = rec.attributes.iter().collect();
        let n = attrs_with_values.len();

        if n > 0 {
            writeln!(w, "  :additionalProperty")?;
            for (i, attr) in attrs_with_values.iter().enumerate() {
                let suffix = if i < n - 1 { "," } else { "." };
                writeln!(w, "    <{}> {}", attr.property_iri(acc), suffix)?;
            }

            writeln!(w)?;

            for attr in &attrs_with_values {
                let name = attr.preferred_name();
                let name_escaped = Self::escape_turtle_string(name);
                writeln!(w, "<{}> a :PropertyValue ;", attr.property_iri(acc))?;
                writeln!(w, "  :name \"{}\" ;", name_escaped)?;
                match &attr.value {
                    Some(v) => {
                        let v_escaped = Self::escape_turtle_string(v);
                        writeln!(w, "  :value \"{}\" .", v_escaped)?;
                    }
                    None => {
                        writeln!(w, "  :value \"\" .")?;
                    }
                }
            }
        } else {
            // Close the record with no additionalProperty
            // Replace last ";" with "."
            // Since we always have at least dct:identifier, we need to end properly
            // Actually the dates above end with ";", we need the last statement to end with "."
            // This is tricky — let's just add a final line
            writeln!(w, "  .")?;
        }

        Ok(())
    }

    fn write_footer<W: Write>(&self, _w: &mut W) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Attribute, BioSampleRecord};

    fn make_record() -> BioSampleRecord {
        BioSampleRecord {
            accession: "SAMD00000345".to_string(),
            submission_date: Some("2014-07-30T00:00:00Z".to_string()),
            last_update: Some("2019-07-23T08:53:00.555Z".to_string()),
            publication_date: Some("2014-07-30T00:00:00Z".to_string()),
            title: Some("type strain of Lactobacillus oryzae".to_string()),
            attributes: vec![
                Attribute {
                    attribute_name: "organism".to_string(),
                    harmonized_name: Some("organism".to_string()),
                    display_name: None,
                    value: Some("Lactobacillus oryzae JCM 18671".to_string()),
                },
                Attribute {
                    attribute_name: "strain".to_string(),
                    harmonized_name: Some("strain".to_string()),
                    display_name: None,
                    value: Some("SG293".to_string()),
                },
            ],
        }
    }

    #[test]
    fn test_turtle_header() {
        let ser = TurtleSerializer::new();
        let mut buf = Vec::new();
        ser.write_header(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("@prefix : <http://schema.org/> ."));
        assert!(output.contains("@prefix idorg: <http://identifiers.org/biosample/> ."));
        assert!(output.contains("@prefix ddbjont:"));
    }

    #[test]
    fn test_turtle_record() {
        let ser = TurtleSerializer::new();
        let rec = make_record();
        let mut buf = Vec::new();
        ser.write_record(&mut buf, &rec).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("idorg:SAMD00000345"));
        assert!(output.contains("a ddbjont:BioSampleRecord"));
        assert!(output.contains("dct:identifier \"SAMD00000345\""));
        assert!(output.contains("dct:description \"type strain of Lactobacillus oryzae\""));
        assert!(output.contains("rdfs:label \"type strain of Lactobacillus oryzae\""));
        assert!(output.contains("dct:created \"2014-07-30T00:00:00Z\"^^xsd:dateTime"));
        assert!(output.contains("dct:modified \"2019-07-23T08:53:00.555Z\"^^xsd:dateTime"));
        assert!(output.contains(":additionalProperty"));
        assert!(output.contains(":name \"organism\""));
        assert!(output.contains(":value \"Lactobacillus oryzae JCM 18671\""));
        assert!(output.contains(":name \"strain\""));
        assert!(output.contains(":value \"SG293\""));
    }

    #[test]
    fn test_turtle_escapes_special_chars() {
        let ser = TurtleSerializer::new();
        let rec = BioSampleRecord {
            accession: "SAMN00000001".to_string(),
            submission_date: None,
            last_update: None,
            publication_date: None,
            title: Some("test with \"quotes\" and \\backslash".to_string()),
            attributes: vec![Attribute {
                attribute_name: "note".to_string(),
                harmonized_name: None,
                display_name: None,
                value: Some("line1\nline2".to_string()),
            }],
        };
        let mut buf = Vec::new();
        ser.write_record(&mut buf, &rec).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains(r#"test with \"quotes\" and \\backslash"#));
        assert!(output.contains(r#"line1\nline2"#));
    }

    #[test]
    fn test_turtle_empty_attributes() {
        let ser = TurtleSerializer::new();
        let rec = BioSampleRecord {
            accession: "SAMN00000001".to_string(),
            submission_date: None,
            last_update: None,
            publication_date: None,
            title: None,
            attributes: vec![],
        };
        let mut buf = Vec::new();
        ser.write_record(&mut buf, &rec).unwrap();
        let output = String::from_utf8(buf).unwrap();

        assert!(output.contains("idorg:SAMN00000001"));
        assert!(!output.contains(":additionalProperty"));
    }
}
```

- [ ] **Step 3: Add module to lib.rs**

Add to `src/lib.rs`:

```rust
pub mod error;
pub mod model;
pub mod parser;
pub mod serializer;
```

- [ ] **Step 4: Create stub files for ntriples and jsonld**

Create `src/serializer/ntriples.rs`:

```rust
use crate::model::BioSampleRecord;
use crate::serializer::Serializer;
use std::io::Write;

pub struct NTriplesSerializer;

impl Serializer for NTriplesSerializer {
    fn write_header<W: Write>(&self, _w: &mut W) -> std::io::Result<()> {
        Ok(())
    }

    fn write_record<W: Write>(&self, _w: &mut W, _record: &BioSampleRecord) -> std::io::Result<()> {
        todo!("N-Triples serializer not yet implemented")
    }

    fn write_footer<W: Write>(&self, _w: &mut W) -> std::io::Result<()> {
        Ok(())
    }
}
```

Create `src/serializer/jsonld.rs`:

```rust
use crate::model::BioSampleRecord;
use crate::serializer::Serializer;
use std::io::Write;

pub struct JsonLdSerializer;

impl Serializer for JsonLdSerializer {
    fn write_header<W: Write>(&self, _w: &mut W) -> std::io::Result<()> {
        Ok(())
    }

    fn write_record<W: Write>(&self, _w: &mut W, _record: &BioSampleRecord) -> std::io::Result<()> {
        todo!("JSON-LD serializer not yet implemented")
    }

    fn write_footer<W: Write>(&self, _w: &mut W) -> std::io::Result<()> {
        Ok(())
    }
}
```

- [ ] **Step 5: Run tests**

Run:
```bash
cd ~/repos/biosample-rdf && cargo test serializer::turtle
```

Expected: All Turtle serializer tests pass.

- [ ] **Step 6: Commit**

```bash
cd ~/repos/biosample-rdf
git add src/serializer/ src/lib.rs
git commit -m "feat: add Turtle serializer with string escaping"
```

---

### Task 6: N-Triples Serializer

**Files:**
- Modify: `src/serializer/ntriples.rs`

- [ ] **Step 1: Implement N-Triples serializer with tests**

Replace `src/serializer/ntriples.rs` with:

```rust
use crate::model::BioSampleRecord;
use crate::serializer::{Serializer, PREFIX_DDBJONT, PREFIX_DCT, PREFIX_IDORG, PREFIX_RDFS, PREFIX_SCHEMA, PREFIX_XSD};
use std::io::Write;

pub struct NTriplesSerializer;

impl NTriplesSerializer {
    pub fn new() -> Self {
        Self
    }

    fn escape_ntriples_string(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    }

    fn write_triple<W: Write>(
        w: &mut W,
        subject: &str,
        predicate: &str,
        object: &str,
    ) -> std::io::Result<()> {
        writeln!(w, "{} {} {} .", subject, predicate, object)
    }
}

impl Serializer for NTriplesSerializer {
    fn write_header<W: Write>(&self, _w: &mut W) -> std::io::Result<()> {
        Ok(()) // N-Triples has no header
    }

    fn write_record<W: Write>(&self, w: &mut W, rec: &BioSampleRecord) -> std::io::Result<()> {
        let subj = format!("<{}{}>", PREFIX_IDORG, rec.accession);
        let desc = rec.title.as_deref().unwrap_or("");
        let desc_escaped = Self::escape_ntriples_string(desc);

        // rdf:type
        Self::write_triple(
            w,
            &subj,
            "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>",
            &format!("<{}BioSampleRecord>", PREFIX_DDBJONT),
        )?;

        // dct:identifier
        Self::write_triple(
            w,
            &subj,
            &format!("<{}identifier>", PREFIX_DCT),
            &format!("\"{}\"", rec.accession),
        )?;

        // dct:description
        Self::write_triple(
            w,
            &subj,
            &format!("<{}description>", PREFIX_DCT),
            &format!("\"{}\"", desc_escaped),
        )?;

        // rdfs:label
        Self::write_triple(
            w,
            &subj,
            &format!("<{}label>", PREFIX_RDFS),
            &format!("\"{}\"", desc_escaped),
        )?;

        // Dates
        if let Some(ref d) = rec.submission_date {
            Self::write_triple(
                w,
                &subj,
                &format!("<{}created>", PREFIX_DCT),
                &format!("\"{}\"^^<{}dateTime>", d, PREFIX_XSD),
            )?;
        }
        if let Some(ref d) = rec.last_update {
            Self::write_triple(
                w,
                &subj,
                &format!("<{}modified>", PREFIX_DCT),
                &format!("\"{}\"^^<{}dateTime>", d, PREFIX_XSD),
            )?;
        }
        if let Some(ref d) = rec.publication_date {
            Self::write_triple(
                w,
                &subj,
                &format!("<{}issued>", PREFIX_DCT),
                &format!("\"{}\"^^<{}dateTime>", d, PREFIX_XSD),
            )?;
        }

        // Attributes
        for attr in &rec.attributes {
            let prop_iri = format!("<{}>", attr.property_iri(&rec.accession));

            // Link record to property
            Self::write_triple(
                w,
                &subj,
                &format!("<{}additionalProperty>", PREFIX_SCHEMA),
                &prop_iri,
            )?;

            // Property type
            Self::write_triple(
                w,
                &prop_iri,
                "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>",
                &format!("<{}PropertyValue>", PREFIX_SCHEMA),
            )?;

            // Property name
            let name_escaped = Self::escape_ntriples_string(attr.preferred_name());
            Self::write_triple(
                w,
                &prop_iri,
                &format!("<{}name>", PREFIX_SCHEMA),
                &format!("\"{}\"", name_escaped),
            )?;

            // Property value
            let val = match &attr.value {
                Some(v) => Self::escape_ntriples_string(v),
                None => String::new(),
            };
            Self::write_triple(
                w,
                &prop_iri,
                &format!("<{}value>", PREFIX_SCHEMA),
                &format!("\"{}\"", val),
            )?;
        }

        Ok(())
    }

    fn write_footer<W: Write>(&self, _w: &mut W) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Attribute, BioSampleRecord};

    fn make_record() -> BioSampleRecord {
        BioSampleRecord {
            accession: "SAMD00000345".to_string(),
            submission_date: Some("2014-07-30T00:00:00Z".to_string()),
            last_update: None,
            publication_date: None,
            title: Some("test organism".to_string()),
            attributes: vec![Attribute {
                attribute_name: "organism".to_string(),
                harmonized_name: Some("organism".to_string()),
                display_name: None,
                value: Some("Homo sapiens".to_string()),
            }],
        }
    }

    #[test]
    fn test_ntriples_record() {
        let ser = NTriplesSerializer::new();
        let rec = make_record();
        let mut buf = Vec::new();
        ser.write_record(&mut buf, &rec).unwrap();
        let output = String::from_utf8(buf).unwrap();

        // Each line must end with " ."
        for line in output.lines() {
            assert!(line.ends_with(" ."), "Line does not end with ' .': {}", line);
        }

        // Check key triples
        assert!(output.contains("<http://identifiers.org/biosample/SAMD00000345>"));
        assert!(output.contains("<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>"));
        assert!(output.contains("<http://ddbj.nig.ac.jp/ontologies/biosample/BioSampleRecord>"));
        assert!(output.contains("\"SAMD00000345\""));
        assert!(output.contains("\"Homo sapiens\""));
    }

    #[test]
    fn test_ntriples_no_header() {
        let ser = NTriplesSerializer::new();
        let mut buf = Vec::new();
        ser.write_header(&mut buf).unwrap();
        assert!(buf.is_empty());
    }
}
```

- [ ] **Step 2: Run tests**

Run:
```bash
cd ~/repos/biosample-rdf && cargo test serializer::ntriples
```

Expected: All N-Triples tests pass.

- [ ] **Step 3: Commit**

```bash
cd ~/repos/biosample-rdf
git add src/serializer/ntriples.rs
git commit -m "feat: add N-Triples serializer"
```

---

### Task 7: JSON-LD Serializer

**Files:**
- Modify: `src/serializer/jsonld.rs`

- [ ] **Step 1: Implement JSON-LD serializer with tests**

Replace `src/serializer/jsonld.rs` with:

```rust
use crate::model::BioSampleRecord;
use crate::serializer::Serializer;
use serde::Serialize;
use std::io::Write;

pub struct JsonLdSerializer {
    first_record: bool,
}

impl JsonLdSerializer {
    pub fn new() -> Self {
        Self { first_record: true }
    }
}

#[derive(Serialize)]
struct JsonLdContext {
    #[serde(rename = "@base")]
    base: &'static str,
    #[serde(rename = "@vocab")]
    vocab: &'static str,
}

#[derive(Serialize)]
struct JsonLdRecord {
    #[serde(rename = "@context")]
    context: JsonLdContext,
    #[serde(rename = "@type")]
    record_type: &'static str,
    #[serde(rename = "@id")]
    id: String,
    #[serde(rename = "dct:identifier")]
    identifier: String,
    #[serde(rename = "dct:description", skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(rename = "rdfs:label", skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(rename = "dct:created", skip_serializing_if = "Option::is_none")]
    created: Option<JsonLdDateTime>,
    #[serde(rename = "dct:modified", skip_serializing_if = "Option::is_none")]
    modified: Option<JsonLdDateTime>,
    #[serde(rename = "dct:issued", skip_serializing_if = "Option::is_none")]
    issued: Option<JsonLdDateTime>,
    #[serde(rename = "additionalProperty", skip_serializing_if = "Vec::is_empty")]
    additional_property: Vec<JsonLdProperty>,
}

#[derive(Serialize)]
struct JsonLdDateTime {
    #[serde(rename = "@value")]
    value: String,
    #[serde(rename = "@type")]
    dtype: &'static str,
}

#[derive(Serialize)]
struct JsonLdProperty {
    #[serde(rename = "@type")]
    prop_type: &'static str,
    #[serde(rename = "@id")]
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
}

impl Serializer for JsonLdSerializer {
    fn write_header<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        writeln!(w, "[")?;
        Ok(())
    }

    fn write_record<W: Write>(&self, w: &mut W, rec: &BioSampleRecord) -> std::io::Result<()> {
        let jsonld = JsonLdRecord {
            context: JsonLdContext {
                base: "http://schema.org/",
                vocab: "http://schema.org/",
            },
            record_type: "ddbjont:BioSampleRecord",
            id: rec.iri(),
            identifier: rec.accession.clone(),
            description: rec.title.clone(),
            label: rec.title.clone(),
            created: rec.submission_date.as_ref().map(|d| JsonLdDateTime {
                value: d.clone(),
                dtype: "xsd:dateTime",
            }),
            modified: rec.last_update.as_ref().map(|d| JsonLdDateTime {
                value: d.clone(),
                dtype: "xsd:dateTime",
            }),
            issued: rec.publication_date.as_ref().map(|d| JsonLdDateTime {
                value: d.clone(),
                dtype: "xsd:dateTime",
            }),
            additional_property: rec
                .attributes
                .iter()
                .map(|attr| JsonLdProperty {
                    prop_type: "PropertyValue",
                    id: attr.property_iri(&rec.accession),
                    name: attr.preferred_name().to_string(),
                    value: attr.value.clone(),
                })
                .collect(),
        };

        let json = serde_json::to_string_pretty(&jsonld)?;
        // Write comma separator between records
        // The caller handles this through the mutable first_record flag
        w.write_all(json.as_bytes())?;
        writeln!(w)?;
        Ok(())
    }

    fn write_footer<W: Write>(&self, w: &mut W) -> std::io::Result<()> {
        writeln!(w, "]")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Attribute, BioSampleRecord};

    fn make_record() -> BioSampleRecord {
        BioSampleRecord {
            accession: "SAMD00000345".to_string(),
            submission_date: Some("2014-07-30T00:00:00Z".to_string()),
            last_update: None,
            publication_date: None,
            title: Some("test organism".to_string()),
            attributes: vec![Attribute {
                attribute_name: "organism".to_string(),
                harmonized_name: Some("organism".to_string()),
                display_name: None,
                value: Some("Homo sapiens".to_string()),
            }],
        }
    }

    #[test]
    fn test_jsonld_record_is_valid_json() {
        let ser = JsonLdSerializer::new();
        let rec = make_record();
        let mut buf = Vec::new();
        ser.write_record(&mut buf, &rec).unwrap();
        let output = String::from_utf8(buf).unwrap();

        // Should parse as valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["@id"], "http://identifiers.org/biosample/SAMD00000345");
        assert_eq!(parsed["dct:identifier"], "SAMD00000345");
        assert_eq!(parsed["dct:description"], "test organism");
        assert_eq!(parsed["additionalProperty"][0]["name"], "organism");
        assert_eq!(parsed["additionalProperty"][0]["value"], "Homo sapiens");
    }

    #[test]
    fn test_jsonld_has_context() {
        let ser = JsonLdSerializer::new();
        let rec = make_record();
        let mut buf = Vec::new();
        ser.write_record(&mut buf, &rec).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["@context"]["@base"], "http://schema.org/");
        assert_eq!(parsed["@context"]["@vocab"], "http://schema.org/");
    }

    #[test]
    fn test_jsonld_skips_none_dates() {
        let ser = JsonLdSerializer::new();
        let rec = BioSampleRecord {
            accession: "SAMN00000001".to_string(),
            submission_date: None,
            last_update: None,
            publication_date: None,
            title: None,
            attributes: vec![],
        };
        let mut buf = Vec::new();
        ser.write_record(&mut buf, &rec).unwrap();
        let output = String::from_utf8(buf).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(parsed.get("dct:created").is_none());
        assert!(parsed.get("dct:modified").is_none());
        assert!(parsed.get("dct:description").is_none());
    }

    #[test]
    fn test_jsonld_header_footer() {
        let ser = JsonLdSerializer::new();
        let mut buf = Vec::new();
        ser.write_header(&mut buf).unwrap();
        let header = String::from_utf8(buf).unwrap();
        assert!(header.contains("["));

        let mut buf = Vec::new();
        ser.write_footer(&mut buf).unwrap();
        let footer = String::from_utf8(buf).unwrap();
        assert!(footer.contains("]"));
    }
}
```

- [ ] **Step 2: Run tests**

Run:
```bash
cd ~/repos/biosample-rdf && cargo test serializer::jsonld
```

Expected: All JSON-LD tests pass.

- [ ] **Step 3: Commit**

```bash
cd ~/repos/biosample-rdf
git add src/serializer/jsonld.rs
git commit -m "feat: add JSON-LD serializer"
```

---

### Task 8: Chunk Writer & Progress Tracking

**Files:**
- Create: `src/chunk.rs`
- Create: `src/progress.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create progress tracking**

Create `src/progress.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    pub source_file: String,
    pub source_size: u64,
    pub source_md5: String,
    pub chunks_completed: u32,
    pub records_processed: u64,
    pub records_skipped: u64,
    pub started_at: String,
}

impl Progress {
    pub fn new(source_file: &str, source_size: u64, source_md5: &str) -> Self {
        Self {
            source_file: source_file.to_string(),
            source_size,
            source_md5: source_md5.to_string(),
            chunks_completed: 0,
            records_processed: 0,
            records_skipped: 0,
            started_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let data = fs::read_to_string(path)?;
        let progress: Self = serde_json::from_str(&data)?;
        Ok(progress)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_progress_save_and_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("progress.json");

        let mut progress = Progress::new("test.xml.gz", 1000, "abc123");
        progress.chunks_completed = 5;
        progress.records_processed = 500000;
        progress.records_skipped = 3;
        progress.save(&path).unwrap();

        let loaded = Progress::load(&path).unwrap();
        assert_eq!(loaded.source_file, "test.xml.gz");
        assert_eq!(loaded.source_size, 1000);
        assert_eq!(loaded.source_md5, "abc123");
        assert_eq!(loaded.chunks_completed, 5);
        assert_eq!(loaded.records_processed, 500000);
        assert_eq!(loaded.records_skipped, 3);
    }
}
```

- [ ] **Step 2: Create chunk writer**

Create `src/chunk.rs`:

```rust
use crate::model::BioSampleRecord;
use crate::progress::Progress;
use crate::serializer::jsonld::JsonLdSerializer;
use crate::serializer::ntriples::NTriplesSerializer;
use crate::serializer::turtle::TurtleSerializer;
use crate::serializer::Serializer;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

pub struct ChunkWriter {
    output_dir: PathBuf,
    chunk_size: usize,
    buffer: Vec<BioSampleRecord>,
    progress: Progress,
    ttl_ser: TurtleSerializer,
    nt_ser: NTriplesSerializer,
    jsonld_ser: JsonLdSerializer,
}

impl ChunkWriter {
    pub fn new(
        output_dir: PathBuf,
        chunk_size: usize,
        progress: Progress,
    ) -> std::io::Result<Self> {
        // Create output subdirectories
        fs::create_dir_all(output_dir.join("ttl"))?;
        fs::create_dir_all(output_dir.join("jsonld"))?;
        fs::create_dir_all(output_dir.join("nt"))?;

        Ok(Self {
            output_dir,
            chunk_size,
            buffer: Vec::with_capacity(chunk_size),
            progress,
            ttl_ser: TurtleSerializer::new(),
            nt_ser: NTriplesSerializer::new(),
            jsonld_ser: JsonLdSerializer::new(),
        })
    }

    /// Add a record to the buffer. Flushes if buffer reaches chunk_size.
    pub fn add_record(&mut self, record: BioSampleRecord) -> std::io::Result<()> {
        self.buffer.push(record);
        self.progress.records_processed += 1;

        if self.buffer.len() >= self.chunk_size {
            self.flush_chunk()?;
        }
        Ok(())
    }

    /// Record a skipped (errored) record.
    pub fn record_skip(&mut self) {
        self.progress.records_skipped += 1;
    }

    /// Flush remaining records in the buffer.
    pub fn finish(&mut self) -> std::io::Result<()> {
        if !self.buffer.is_empty() {
            self.flush_chunk()?;
        }

        // Write final manifest
        self.write_manifest()?;
        Ok(())
    }

    fn flush_chunk(&mut self) -> std::io::Result<()> {
        let chunk_num = self.progress.chunks_completed + 1;
        let chunk_name = format!("chunk_{:04}", chunk_num);

        // Write TTL
        let ttl_path = self.output_dir.join("ttl").join(format!("{}.ttl", chunk_name));
        self.write_format(&ttl_path, &self.ttl_ser.clone())?;

        // Write N-Triples
        let nt_path = self.output_dir.join("nt").join(format!("{}.nt", chunk_name));
        self.write_format(&nt_path, &self.nt_ser.clone())?;

        // Write JSON-LD
        let jsonld_path = self.output_dir.join("jsonld").join(format!("{}.jsonld", chunk_name));
        self.write_jsonld(&jsonld_path)?;

        // Update progress
        self.progress.chunks_completed = chunk_num;
        let progress_path = self.output_dir.join("progress.json");
        self.progress.save(&progress_path)?;

        self.buffer.clear();
        Ok(())
    }

    fn write_format<S: Serializer>(&self, path: &Path, ser: &S) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        ser.write_header(&mut writer)?;
        for rec in &self.buffer {
            ser.write_record(&mut writer, rec)?;
        }
        ser.write_footer(&mut writer)?;
        Ok(())
    }

    fn write_jsonld(&self, path: &Path) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        let ser = JsonLdSerializer::new();
        ser.write_header(&mut writer)?;
        for (i, rec) in self.buffer.iter().enumerate() {
            if i > 0 {
                write!(writer, ",")?;
            }
            ser.write_record(&mut writer, rec)?;
        }
        ser.write_footer(&mut writer)?;
        Ok(())
    }

    fn write_manifest(&self) -> std::io::Result<()> {
        let manifest = serde_json::json!({
            "source_file": self.progress.source_file,
            "records_processed": self.progress.records_processed,
            "records_skipped": self.progress.records_skipped,
            "chunks": self.progress.chunks_completed,
            "completed_at": chrono::Utc::now().to_rfc3339(),
        });
        let path = self.output_dir.join("manifest.json");
        let json = serde_json::to_string_pretty(&manifest)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn progress(&self) -> &Progress {
        &self.progress
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Attribute, BioSampleRecord};
    use tempfile::TempDir;

    fn make_record(accession: &str) -> BioSampleRecord {
        BioSampleRecord {
            accession: accession.to_string(),
            submission_date: Some("2020-01-01T00:00:00Z".to_string()),
            last_update: None,
            publication_date: None,
            title: Some(format!("Sample {}", accession)),
            attributes: vec![Attribute {
                attribute_name: "organism".to_string(),
                harmonized_name: Some("organism".to_string()),
                display_name: None,
                value: Some("Test organism".to_string()),
            }],
        }
    }

    #[test]
    fn test_chunk_writer_creates_output_dirs() {
        let dir = TempDir::new().unwrap();
        let progress = Progress::new("test.xml.gz", 100, "md5");
        let _writer = ChunkWriter::new(dir.path().to_path_buf(), 10, progress).unwrap();

        assert!(dir.path().join("ttl").exists());
        assert!(dir.path().join("jsonld").exists());
        assert!(dir.path().join("nt").exists());
    }

    #[test]
    fn test_chunk_writer_flushes_at_chunk_size() {
        let dir = TempDir::new().unwrap();
        let progress = Progress::new("test.xml.gz", 100, "md5");
        let mut writer = ChunkWriter::new(dir.path().to_path_buf(), 3, progress).unwrap();

        for i in 0..3 {
            writer.add_record(make_record(&format!("SAMN{:08}", i))).unwrap();
        }

        // After 3 records with chunk_size=3, should have flushed
        assert!(dir.path().join("ttl/chunk_0001.ttl").exists());
        assert!(dir.path().join("jsonld/chunk_0001.jsonld").exists());
        assert!(dir.path().join("nt/chunk_0001.nt").exists());
        assert!(dir.path().join("progress.json").exists());
        assert_eq!(writer.progress().chunks_completed, 1);
        assert_eq!(writer.progress().records_processed, 3);
    }

    #[test]
    fn test_chunk_writer_finish_flushes_remaining() {
        let dir = TempDir::new().unwrap();
        let progress = Progress::new("test.xml.gz", 100, "md5");
        let mut writer = ChunkWriter::new(dir.path().to_path_buf(), 100, progress).unwrap();

        writer.add_record(make_record("SAMN00000001")).unwrap();
        writer.add_record(make_record("SAMN00000002")).unwrap();
        writer.finish().unwrap();

        assert!(dir.path().join("ttl/chunk_0001.ttl").exists());
        assert!(dir.path().join("manifest.json").exists());
        assert_eq!(writer.progress().records_processed, 2);
    }

    #[test]
    fn test_chunk_writer_ttl_content() {
        let dir = TempDir::new().unwrap();
        let progress = Progress::new("test.xml.gz", 100, "md5");
        let mut writer = ChunkWriter::new(dir.path().to_path_buf(), 100, progress).unwrap();

        writer.add_record(make_record("SAMD00000345")).unwrap();
        writer.finish().unwrap();

        let ttl = fs::read_to_string(dir.path().join("ttl/chunk_0001.ttl")).unwrap();
        assert!(ttl.contains("@prefix"));
        assert!(ttl.contains("idorg:SAMD00000345"));
        assert!(ttl.contains("ddbjont:BioSampleRecord"));
    }
}
```

- [ ] **Step 3: Add modules to lib.rs**

Update `src/lib.rs`:

```rust
pub mod error;
pub mod model;
pub mod parser;
pub mod serializer;
pub mod chunk;
pub mod progress;
```

- [ ] **Step 4: Make serializers Clone**

Add `#[derive(Clone)]` to `TurtleSerializer` in `src/serializer/turtle.rs`:

```rust
#[derive(Clone)]
pub struct TurtleSerializer;
```

Add `#[derive(Clone)]` to `NTriplesSerializer` in `src/serializer/ntriples.rs`:

```rust
#[derive(Clone)]
pub struct NTriplesSerializer;
```

Add `#[derive(Clone)]` to `JsonLdSerializer` in `src/serializer/jsonld.rs`:

```rust
#[derive(Clone)]
pub struct JsonLdSerializer {
    first_record: bool,
}
```

- [ ] **Step 5: Run tests**

Run:
```bash
cd ~/repos/biosample-rdf && cargo test
```

Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
cd ~/repos/biosample-rdf
git add src/chunk.rs src/progress.rs src/lib.rs src/serializer/turtle.rs src/serializer/ntriples.rs src/serializer/jsonld.rs
git commit -m "feat: add chunk writer and progress tracking"
```

---

### Task 9: Wire Up the Convert Command

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Implement the convert command**

Replace the `main()` function in `src/main.rs`:

```rust
use biosample_rdf::chunk::ChunkWriter;
use biosample_rdf::parser::BioSampleParser;
use biosample_rdf::progress::Progress;

use clap::{Parser, Subcommand};
use flate2::read::GzDecoder;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
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
        /// Path to biosample_set.xml.gz (or uncompressed .xml) input file
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

fn compute_md5(path: &PathBuf) -> anyhow::Result<String> {
    use md5::{Md5, Digest};
    use std::io::Read;

    let mut file = File::open(path)?;
    let mut hasher = Md5::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn run_convert(input: PathBuf, output_dir: PathBuf, chunk_size: usize) -> anyhow::Result<()> {
    // Open error log
    std::fs::create_dir_all(&output_dir)?;
    let error_log_path = output_dir.join("errors.log");
    let mut error_log = BufWriter::new(File::create(&error_log_path)?);

    // Compute source metadata
    let source_size = std::fs::metadata(&input)?.len();
    eprintln!("Computing MD5 of input file...");
    let source_md5 = compute_md5(&input)?;
    eprintln!("Input: {:?} ({} bytes, md5: {})", input, source_size, source_md5);

    let progress = Progress::new(
        input.to_str().unwrap_or("unknown"),
        source_size,
        &source_md5,
    );

    let mut chunk_writer = ChunkWriter::new(output_dir, chunk_size, progress)?;

    // Open input — handle both .gz and plain .xml
    let file = File::open(&input)?;
    let is_gzipped = input
        .extension()
        .map(|e| e == "gz")
        .unwrap_or(false);

    let reader: Box<dyn std::io::Read> = if is_gzipped {
        Box::new(GzDecoder::new(file))
    } else {
        Box::new(file)
    };

    let buf_reader = BufReader::with_capacity(64 * 1024, reader);
    let mut parser = BioSampleParser::new(buf_reader);

    eprintln!("Starting conversion (chunk size: {})...", chunk_size);

    loop {
        match parser.next_record() {
            Ok(Some(record)) => {
                chunk_writer.add_record(record)?;
                let p = chunk_writer.progress();
                if p.records_processed % 100_000 == 0 {
                    eprintln!(
                        "  Processed {} records ({} chunks, {} skipped)",
                        p.records_processed, p.chunks_completed, p.records_skipped
                    );
                }
            }
            Ok(None) => break,
            Err(e) => {
                writeln!(error_log, "{}", e)?;
                chunk_writer.record_skip();
            }
        }
    }

    chunk_writer.finish()?;

    let p = chunk_writer.progress();
    eprintln!(
        "Done. {} records processed, {} skipped, {} chunks written.",
        p.records_processed, p.records_skipped, p.chunks_completed
    );

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Convert { input, output_dir, chunk_size } => {
            run_convert(input, output_dir, chunk_size)?;
        }
        Commands::Resume { output_dir } => {
            eprintln!("Resuming from {:?}", output_dir);
            todo!("resume not yet implemented")
        }
        Commands::Validate { path } => {
            eprintln!("Validating {:?}", path);
            todo!("validate not yet implemented")
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Verify it compiles**

Run:
```bash
cd ~/repos/biosample-rdf && cargo build
```

Expected: Compiles successfully.

- [ ] **Step 3: Test with the 10-record fixture**

Run:
```bash
cd ~/repos/biosample-rdf && cargo run -- convert --input tests/fixtures/biosample_set.xml.10 --output-dir /tmp/biosample-rdf-test --chunk-size 5
```

Expected: Creates `/tmp/biosample-rdf-test/` with `ttl/chunk_0001.ttl`, `ttl/chunk_0002.ttl`, and corresponding jsonld/nt files. Progress shows 10 records, 0 skipped, 2 chunks.

- [ ] **Step 4: Inspect output**

Run:
```bash
head -30 /tmp/biosample-rdf-test/ttl/chunk_0001.ttl
cat /tmp/biosample-rdf-test/progress.json
cat /tmp/biosample-rdf-test/manifest.json
wc -l /tmp/biosample-rdf-test/nt/chunk_0001.nt
```

Expected: TTL has correct prefixes and record structure. Progress shows completion. N-Triples has multiple lines per record.

- [ ] **Step 5: Test with malformed fixture**

Run:
```bash
cd ~/repos/biosample-rdf && cargo run -- convert --input tests/fixtures/malformed_record.xml --output-dir /tmp/biosample-rdf-malformed --chunk-size 100
```

Expected: 2 records processed, 1 skipped. `errors.log` contains the missing accession error.

Run:
```bash
cat /tmp/biosample-rdf-malformed/errors.log
```

Expected: One line about missing accession.

- [ ] **Step 6: Commit**

```bash
cd ~/repos/biosample-rdf
git add src/main.rs
git commit -m "feat: wire up convert command with streaming pipeline"
```

---

### Task 10: Validate Subcommand

**Files:**
- Create: `src/validate.rs`
- Modify: `src/main.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Implement validator**

Create `src/validate.rs`:

```rust
use std::fs;
use std::path::Path;

/// Validation result for a single file.
#[derive(Debug)]
pub struct ValidationResult {
    pub file: String,
    pub errors: Vec<String>,
    pub record_count: usize,
}

/// Validate a Turtle file for basic structural correctness.
pub fn validate_turtle(path: &Path) -> ValidationResult {
    let file_name = path.display().to_string();
    let mut errors = Vec::new();
    let mut record_count = 0;

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            return ValidationResult {
                file: file_name,
                errors: vec![format!("Cannot read file: {}", e)],
                record_count: 0,
            };
        }
    };

    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('@') {
            continue;
        }

        // Count records by looking for "a ddbjont:BioSampleRecord"
        if line.contains("a ddbjont:BioSampleRecord") {
            record_count += 1;
        }

        // Check for empty identifiers
        if line.contains("dct:identifier \"\"") {
            errors.push(format!("Line {}: empty dct:identifier", line_num + 1));
        }

        // Check for malformed IRIs (very basic)
        if line.starts_with("idorg:") && !line.contains("idorg:SAM") {
            errors.push(format!(
                "Line {}: record IRI doesn't start with SAM prefix: {}",
                line_num + 1,
                line
            ));
        }
    }

    ValidationResult {
        file: file_name,
        errors,
        record_count,
    }
}

/// Validate a N-Triples file for basic structural correctness.
pub fn validate_ntriples(path: &Path) -> ValidationResult {
    let file_name = path.display().to_string();
    let mut errors = Vec::new();
    let mut record_count = 0;

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            return ValidationResult {
                file: file_name,
                errors: vec![format!("Cannot read file: {}", e)],
                record_count: 0,
            };
        }
    };

    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Every N-Triples line must end with " ."
        if !line.ends_with(" .") {
            errors.push(format!("Line {}: does not end with ' .'", line_num + 1));
        }

        // Count records by rdf:type BioSampleRecord triples
        if line.contains("BioSampleRecord") && line.contains("22-rdf-syntax-ns#type") {
            record_count += 1;
        }
    }

    ValidationResult {
        file: file_name,
        errors,
        record_count,
    }
}

/// Validate a directory of output files.
pub fn validate_directory(dir: &Path) -> Vec<ValidationResult> {
    let mut results = Vec::new();

    let ttl_dir = dir.join("ttl");
    if ttl_dir.exists() {
        if let Ok(entries) = fs::read_dir(&ttl_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().map(|e| e == "ttl").unwrap_or(false) {
                    results.push(validate_turtle(&entry.path()));
                }
            }
        }
    }

    let nt_dir = dir.join("nt");
    if nt_dir.exists() {
        if let Ok(entries) = fs::read_dir(&nt_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().map(|e| e == "nt").unwrap_or(false) {
                    results.push(validate_ntriples(&entry.path()));
                }
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_validate_valid_turtle() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.ttl");
        fs::write(
            &path,
            r#"@prefix : <http://schema.org/> .
@prefix idorg: <http://identifiers.org/biosample/> .
@prefix ddbjont: <http://ddbj.nig.ac.jp/ontologies/biosample/> .
@prefix dct: <http://purl.org/dc/terms/> .

idorg:SAMN00000001
  a ddbjont:BioSampleRecord ;
  dct:identifier "SAMN00000001" .
"#,
        )
        .unwrap();

        let result = validate_turtle(&path);
        assert_eq!(result.record_count, 1);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_turtle_empty_identifier() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.ttl");
        fs::write(
            &path,
            r#"@prefix dct: <http://purl.org/dc/terms/> .
  dct:identifier "" .
"#,
        )
        .unwrap();

        let result = validate_turtle(&path);
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].contains("empty dct:identifier"));
    }

    #[test]
    fn test_validate_valid_ntriples() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.nt");
        fs::write(
            &path,
            "<http://identifiers.org/biosample/SAMN00000001> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://ddbj.nig.ac.jp/ontologies/biosample/BioSampleRecord> .\n",
        )
        .unwrap();

        let result = validate_ntriples(&path);
        assert_eq!(result.record_count, 1);
        assert!(result.errors.is_empty());
    }
}
```

- [ ] **Step 2: Wire validate into main.rs**

In `src/main.rs`, replace the `Commands::Validate` match arm:

```rust
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

            eprintln!(
                "Validation complete: {} files, {} records, {} errors",
                results.len(),
                total_records,
                total_errors
            );

            if total_errors > 0 {
                std::process::exit(1);
            }
        }
```

- [ ] **Step 3: Add module to lib.rs**

Add `pub mod validate;` to `src/lib.rs`.

- [ ] **Step 4: Run tests**

Run:
```bash
cd ~/repos/biosample-rdf && cargo test
```

Expected: All tests pass.

- [ ] **Step 5: Test validate on real output**

Run:
```bash
cd ~/repos/biosample-rdf && cargo run -- validate /tmp/biosample-rdf-test/
```

Expected: Shows OK for all chunk files with record counts, exit code 0.

- [ ] **Step 6: Commit**

```bash
cd ~/repos/biosample-rdf
git add src/validate.rs src/main.rs src/lib.rs
git commit -m "feat: add validate subcommand for output verification"
```

---

### Task 11: Golden File Tests

**Files:**
- Create: `tests/golden_test.rs`

- [ ] **Step 1: Write golden file test**

Create `tests/golden_test.rs`:

```rust
use biosample_rdf::chunk::ChunkWriter;
use biosample_rdf::model::BioSampleRecord;
use biosample_rdf::parser::BioSampleParser;
use biosample_rdf::progress::Progress;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::BufReader;
use tempfile::TempDir;

/// Parse the 10-record fixture and serialize, then check the output
/// contains expected triples for the first record (SAMN00000002).
#[test]
fn test_golden_10_records_ttl() {
    let file = File::open("tests/fixtures/biosample_set.xml.10").expect("fixture not found");
    let reader = BufReader::new(file);
    let mut parser = BioSampleParser::new(reader);

    let dir = TempDir::new().unwrap();
    let progress = Progress::new("test.xml", 0, "test");
    let mut writer = ChunkWriter::new(dir.path().to_path_buf(), 100, progress).unwrap();

    loop {
        match parser.next_record() {
            Ok(Some(rec)) => writer.add_record(rec).unwrap(),
            Ok(None) => break,
            Err(_) => writer.record_skip(),
        }
    }
    writer.finish().unwrap();

    let ttl = fs::read_to_string(dir.path().join("ttl/chunk_0001.ttl")).unwrap();

    // Check first record
    assert!(ttl.contains("idorg:SAMN00000002"));
    assert!(ttl.contains("a ddbjont:BioSampleRecord"));
    assert!(ttl.contains("dct:identifier \"SAMN00000002\""));
    assert!(ttl.contains("dct:description \"Alistipes putredinis DSM 17216\""));
    assert!(ttl.contains("rdfs:label \"Alistipes putredinis DSM 17216\""));

    // Check attributes exist
    assert!(ttl.contains(":name \"strain\""));
    assert!(ttl.contains(":value \"DSM 17216\""));

    // Check we got all 10 records
    let record_count = ttl.matches("a ddbjont:BioSampleRecord").count();
    assert_eq!(record_count, 10, "Expected 10 records, got {}", record_count);
}

#[test]
fn test_golden_10_records_ntriples() {
    let file = File::open("tests/fixtures/biosample_set.xml.10").expect("fixture not found");
    let reader = BufReader::new(file);
    let mut parser = BioSampleParser::new(reader);

    let dir = TempDir::new().unwrap();
    let progress = Progress::new("test.xml", 0, "test");
    let mut writer = ChunkWriter::new(dir.path().to_path_buf(), 100, progress).unwrap();

    loop {
        match parser.next_record() {
            Ok(Some(rec)) => writer.add_record(rec).unwrap(),
            Ok(None) => break,
            Err(_) => writer.record_skip(),
        }
    }
    writer.finish().unwrap();

    let nt = fs::read_to_string(dir.path().join("nt/chunk_0001.nt")).unwrap();

    // Every line must end with " ."
    for line in nt.lines() {
        if !line.trim().is_empty() {
            assert!(line.trim().ends_with(" ."), "Bad N-Triples line: {}", line);
        }
    }

    // Count BioSampleRecord type triples = 10
    let type_count = nt
        .lines()
        .filter(|l| l.contains("BioSampleRecord") && l.contains("#type"))
        .count();
    assert_eq!(type_count, 10);

    // Check first record IRI
    assert!(nt.contains("<http://identifiers.org/biosample/SAMN00000002>"));
}

#[test]
fn test_golden_10_records_jsonld() {
    let file = File::open("tests/fixtures/biosample_set.xml.10").expect("fixture not found");
    let reader = BufReader::new(file);
    let mut parser = BioSampleParser::new(reader);

    let dir = TempDir::new().unwrap();
    let progress = Progress::new("test.xml", 0, "test");
    let mut writer = ChunkWriter::new(dir.path().to_path_buf(), 100, progress).unwrap();

    loop {
        match parser.next_record() {
            Ok(Some(rec)) => writer.add_record(rec).unwrap(),
            Ok(None) => break,
            Err(_) => writer.record_skip(),
        }
    }
    writer.finish().unwrap();

    let jsonld = fs::read_to_string(dir.path().join("jsonld/chunk_0001.jsonld")).unwrap();

    // Should parse as valid JSON array
    let parsed: serde_json::Value = serde_json::from_str(&jsonld).unwrap();
    let arr = parsed.as_array().expect("Expected JSON array");
    assert_eq!(arr.len(), 10, "Expected 10 records in JSON-LD array");

    // First record
    assert_eq!(
        arr[0]["@id"],
        "http://identifiers.org/biosample/SAMN00000002"
    );
    assert_eq!(arr[0]["dct:identifier"], "SAMN00000002");

    // Check all records have @id
    for (i, record) in arr.iter().enumerate() {
        assert!(
            record.get("@id").is_some(),
            "Record {} missing @id",
            i
        );
        assert!(
            record.get("dct:identifier").is_some(),
            "Record {} missing dct:identifier",
            i
        );
    }
}
```

- [ ] **Step 2: Run golden tests**

Run:
```bash
cd ~/repos/biosample-rdf && cargo test golden
```

Expected: All 3 golden tests pass.

- [ ] **Step 3: Commit**

```bash
cd ~/repos/biosample-rdf
git add tests/golden_test.rs
git commit -m "test: add golden file tests for 10-record fixture"
```

---

### Task 12: Gzip Streaming Support Test

**Files:**
- Create: `tests/fixtures/small.xml.gz` (generated)

- [ ] **Step 1: Create a gzipped test fixture**

Run:
```bash
cd ~/repos/biosample-rdf
head -50 tests/fixtures/biosample_set.xml.10 > /tmp/small_biosample.xml
echo "</BioSampleSet>" >> /tmp/small_biosample.xml
gzip -c /tmp/small_biosample.xml > tests/fixtures/small.xml.gz
```

- [ ] **Step 2: Test convert with gzipped input**

Run:
```bash
cd ~/repos/biosample-rdf && cargo run -- convert --input tests/fixtures/small.xml.gz --output-dir /tmp/biosample-rdf-gz
```

Expected: Processes records from the gzipped file without error.

- [ ] **Step 3: Commit**

```bash
cd ~/repos/biosample-rdf
git add tests/fixtures/small.xml.gz
git commit -m "test: add gzip streaming test fixture"
```

---

### Task 13: Dockerfile & Daily Script

**Files:**
- Create: `Dockerfile`
- Create: `scripts/daily_update.sh`

- [ ] **Step 1: Create Dockerfile**

Create `Dockerfile`:

```dockerfile
# Build stage
FROM rust:1-slim AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/biosample-rdf /usr/local/bin/biosample-rdf
WORKDIR /data
ENTRYPOINT ["biosample-rdf"]
```

- [ ] **Step 2: Create daily update script**

Create `scripts/daily_update.sh`:

```bash
#!/bin/bash
set -euo pipefail

# Configuration
DATA_DIR="${DATA_DIR:-/data}"
INPUT_FILE="${DATA_DIR}/biosample_set.xml.gz"
OUTPUT_DIR="${DATA_DIR}/output"
NCBI_URL="https://ftp.ncbi.nlm.nih.gov/biosample/biosample_set.xml.gz"
CHUNK_SIZE="${CHUNK_SIZE:-100000}"

echo "=== BioSample RDF Daily Update ==="
echo "Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "Data dir: ${DATA_DIR}"

# Step 1: Download (skip if unchanged)
echo "--- Step 1: Download ---"
mkdir -p "${DATA_DIR}"
if [ -f "${INPUT_FILE}" ]; then
    echo "Checking for updates..."
    curl -z "${INPUT_FILE}" -o "${INPUT_FILE}" -fSL "${NCBI_URL}"
else
    echo "Downloading full dump..."
    curl -o "${INPUT_FILE}" -fSL "${NCBI_URL}"
fi
echo "Input file: $(ls -lh "${INPUT_FILE}" | awk '{print $5}')"

# Step 2: Convert
echo "--- Step 2: Convert ---"
rm -rf "${OUTPUT_DIR}"
biosample-rdf convert --input "${INPUT_FILE}" --output-dir "${OUTPUT_DIR}" --chunk-size "${CHUNK_SIZE}"

# Step 3: Validate
echo "--- Step 3: Validate ---"
biosample-rdf validate "${OUTPUT_DIR}"

# Step 4: Summary
echo "--- Complete ---"
cat "${OUTPUT_DIR}/manifest.json"

# Step 5: Notify (if webhook URL is set)
if [ -n "${NOTIFY_WEBHOOK:-}" ]; then
    RECORDS=$(jq -r '.records_processed' "${OUTPUT_DIR}/manifest.json")
    SKIPPED=$(jq -r '.records_skipped' "${OUTPUT_DIR}/manifest.json")
    curl -s -X POST "${NOTIFY_WEBHOOK}" \
        -H "Content-Type: application/json" \
        -d "{\"token\": \"${NOTIFY_TOKEN:-}\", \"title\": \"BioSample RDF\", \"message\": \"Done: ${RECORDS} records, ${SKIPPED} skipped\"}" \
        || true
fi
```

- [ ] **Step 3: Make script executable**

Run:
```bash
chmod +x ~/repos/biosample-rdf/scripts/daily_update.sh
```

- [ ] **Step 4: Commit**

```bash
cd ~/repos/biosample-rdf
git add Dockerfile scripts/daily_update.sh
git commit -m "feat: add Dockerfile and daily update script"
```

---

### Task 14: Final Integration Test & Cleanup

**Files:**
- Modify: `src/main.rs` (add `--help` improvements if needed)

- [ ] **Step 1: Run full test suite**

Run:
```bash
cd ~/repos/biosample-rdf && cargo test
```

Expected: All tests pass.

- [ ] **Step 2: Run clippy**

Run:
```bash
cd ~/repos/biosample-rdf && cargo clippy -- -D warnings
```

Expected: No warnings.

- [ ] **Step 3: Fix any clippy issues**

Fix any warnings found by clippy.

- [ ] **Step 4: End-to-end test**

Run:
```bash
cd ~/repos/biosample-rdf
cargo build --release
./target/release/biosample-rdf convert --input tests/fixtures/biosample_set.xml.10 --output-dir /tmp/e2e-test --chunk-size 3
./target/release/biosample-rdf validate /tmp/e2e-test/
```

Expected: 10 records, 4 chunks (3+3+3+1), validation passes.

- [ ] **Step 5: Commit any fixes**

```bash
cd ~/repos/biosample-rdf
git add -A
git commit -m "chore: fix clippy warnings and finalize"
```
