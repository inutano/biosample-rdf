# biosample-rdf: Design Spec

**Date**: 2026-03-31
**Status**: Approved
**Origin**: Reboot of [inutano/biosample_jsonld](https://github.com/inutano/biosample_jsonld)

## Overview

A Rust CLI tool that converts the NCBI BioSample XML dump into three RDF serializations (Turtle, JSON-LD, N-Triples) using a streaming, chunked pipeline. Designed for daily unattended operation with crash resilience and robust error handling for messy XML data.

## Goals

- Process the full NCBI `biosample_set.xml.gz` (~4 GB compressed, ~40-80 GB XML) in under 30 minutes
- Produce TTL, JSON-LD, and N-Triples output for downstream consumers
- Run daily via cron to keep RDF data up-to-date
- Deposit output to filesystem and RDF Portal (rdfportal.org)
- Maintain stable IRI scheme compatible with bsllmner-mk2 annotation layer

## Non-Goals

- Ontology mapping / annotation of attribute values (handled by bsllmner-mk2)
- EBI BioSamples API integration
- SPARQL endpoint hosting

## Data Source

- **Primary**: `https://ftp.ncbi.nlm.nih.gov/biosample/biosample_set.xml.gz`
- Updated daily by NCBI
- Contains all BioSample records in a single `<BioSampleSet>` XML document

## RDF Data Model

Follows the DDBJ model from the original project (schema.org + Dublin Core + DDBJ ontology).

### Prefixes

```turtle
@prefix : <http://schema.org/> .
@prefix idorg: <http://identifiers.org/biosample/> .
@prefix dct: <http://purl.org/dc/terms/> .
@prefix ddbj: <http://ddbj.nig.ac.jp/biosample/> .
@prefix ddbjont: <http://ddbj.nig.ac.jp/ontologies/biosample/> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix xsd: <https://www.w3.org/2001/XMLSchema#> .
```

### IRI Scheme

- **Record IRI**: `idorg:{accession}` → `http://identifiers.org/biosample/{accession}`
- **Attribute property IRI**: `<http://ddbj.nig.ac.jp/biosample/{accession}#{property_name}>` where `property_name` is `harmonized_name` (preferred) or `attribute_name` (fallback), URL-encoded

### Triples per record

```turtle
idorg:SAMD00000345
  a ddbjont:BioSampleRecord ;
  dct:identifier "SAMD00000345" ;
  dct:description "type strain of Lactobacillus oryzae" ;
  rdfs:label "type strain of Lactobacillus oryzae" ;
  dct:created "2014-07-30T00:00:00Z"^^xsd:dateTime ;
  dct:modified "2019-07-23T08:53:00.555Z"^^xsd:dateTime ;
  dct:issued "2014-07-30T00:00:00Z"^^xsd:dateTime ;
  :additionalProperty
    <http://ddbj.nig.ac.jp/biosample/SAMD00000345#organism> ,
    <http://ddbj.nig.ac.jp/biosample/SAMD00000345#strain> .

<http://ddbj.nig.ac.jp/biosample/SAMD00000345#organism> a :PropertyValue ;
  :name "organism" ;
  :value "Lactobacillus oryzae JCM 18671" .

<http://ddbj.nig.ac.jp/biosample/SAMD00000345#strain> a :PropertyValue ;
  :name "strain" ;
  :value "SG293" .
```

### Internal Data Model

```rust
struct BioSampleRecord {
    accession: String,
    submission_date: Option<String>,
    last_update: Option<String>,
    publication_date: Option<String>,
    title: Option<String>,
    attributes: Vec<Attribute>,
}

struct Attribute {
    attribute_name: String,
    harmonized_name: Option<String>,
    display_name: Option<String>,
    value: Option<String>,
}
```

All fields except `accession` and `attribute_name` are `Option` — missing or blank data never causes a crash.

## Pipeline Architecture

### Approach: Single-pass streaming with chunked output

Stream the gzipped XML in a single pass. Buffer records into chunks (default 100,000 records). Flush each chunk to all three output formats. Track progress for crash recovery.

### Flow

```
Download (curl -z)
    ↓
Stream decompress (flate2)
    ↓
SAX pull-parse (quick-xml)
    ↓
Per-record validation
    ↓  (bad record → errors.log, skip)
Chunk buffer (100k records)
    ↓  (buffer full)
Serialize TTL + JSON-LD + N-Triples
    ↓
Write chunk files + update progress.json
```

### Per-Record Error Handling

When a `<BioSample>` element has malformed XML, missing accession, or any parse error:

1. Log the error with context (byte offset, partial accession if available) to `errors.log`
2. Skip the record
3. Continue to the next `<BioSample>` element
4. Never panic or abort the pipeline

### Progress Tracking

`progress.json`:

```json
{
  "source_file": "biosample_set.xml.gz",
  "source_size": 4294967296,
  "source_md5": "abc123...",
  "chunks_completed": 42,
  "records_processed": 4200000,
  "records_skipped": 37,
  "last_byte_offset": 1073741824,
  "started_at": "2026-03-31T00:00:00Z"
}
```

On restart with `resume` subcommand, skip to `last_byte_offset` and continue. If source file md5 changed, start fresh with a warning.

### Output Directory Structure

```
output/
  ttl/chunk_0001.ttl ... chunk_NNNN.ttl
  jsonld/chunk_0001.jsonld ... chunk_NNNN.jsonld
  nt/chunk_0001.nt ... chunk_NNNN.nt
  progress.json
  errors.log
  manifest.json    # final summary: record count, file list, checksums
```

## CLI Interface

```
biosample-rdf convert  --input biosample_set.xml.gz --output-dir ./output [--chunk-size 100000]
biosample-rdf resume   --output-dir ./output
biosample-rdf validate [PATH]   # validate output file(s) or directory
```

### `convert`

Runs the full pipeline from input XML to chunked output files.

### `resume`

Reads `progress.json` from the output directory and resumes from the last completed chunk.

### `validate`

Checks output files for:

- Valid RDF syntax (all triples parseable)
- Every record has expected predicates (`dct:identifier`, `dct:description`, `rdfs:label`, `dct:created`)
- No empty accessions or blank nodes where IRIs are expected
- Record count consistency

Exit code 0 = pass, non-zero = failures (details to stderr).

## Daily Operation

### Cron Workflow

```bash
# 1. Download (skip if unchanged)
curl -z biosample_set.xml.gz -o biosample_set.xml.gz \
  https://ftp.ncbi.nlm.nih.gov/biosample/biosample_set.xml.gz

# 2. Convert
biosample-rdf convert --input biosample_set.xml.gz --output-dir ./output

# 3. Validate
biosample-rdf validate ./output/

# 4. Deposit to RDF Portal
# (mechanism TBD — depends on RDF Portal's submission process)

# 5. Notify
curl -s -X POST https://api.getmoshi.app/api/webhook ...
```

### Containerization

- Multi-stage Dockerfile: build with `rust:slim`, run with `debian:slim`
- Single static binary, no runtime dependencies
- Config via CLI args or environment variables

## Resource Expectations

- **Memory**: ~50-100 MB (streaming parse + chunk buffer)
- **CPU**: single-threaded parse + serialization, CPU-bound but fast in Rust
- **Disk**: output ~3x input size across three formats
- **Wall time target**: under 30 minutes for full dump

## Testing & Validation

### Unit Tests

- XML parser: small XML fragments → verify `BioSampleRecord` fields
- Serializers: given a `BioSampleRecord` → verify valid TTL/JSON-LD/N-Triples syntax
- Error handling: malformed XML → verify graceful skip + error log entry

### Golden File Tests

- Use `example/SAMD00000345.jsonld` and `example/SAMD00000345.ttl` from the old repo as reference
- Extract corresponding record from `example/biosample_set.xml.10` (10-record sample)
- Run pipeline on sample XML
- Compare output as RDF triple sets (not byte-for-byte — ordering/whitespace may differ)

### Edge Case Test Fixtures

From the old repo:
- `quotes.xml` — records with special characters
- `with_blank_attrs.xml` — records with empty attribute values
- `malformed_record.xml` — hand-crafted invalid data (new)

### CI

`cargo test` runs unit + golden file tests. The `validate` subcommand can be used manually or in cron after a full run.

## Project Structure

```
biosample-rdf/
├── Cargo.toml
├── Dockerfile
├── README.md
├── src/
│   ├── main.rs              # CLI entry point (clap)
│   ├── parser.rs            # XML SAX streaming parser (quick-xml)
│   ├── model.rs             # BioSampleRecord, Attribute structs
│   ├── serializer/
│   │   ├── mod.rs
│   │   ├── turtle.rs        # TTL serializer
│   │   ├── jsonld.rs        # JSON-LD serializer
│   │   └── ntriples.rs      # N-Triples serializer
│   ├── chunk.rs             # Chunk buffer + flush logic
│   ├── progress.rs          # Progress tracking + resume
│   ├── validate.rs          # Validation subcommand
│   └── error.rs             # Error types
├── tests/
│   ├── golden_test.rs       # Compare output against reference files
│   ├── parser_test.rs       # XML parse edge cases
│   └── fixtures/
│       ├── biosample_set.xml.10
│       ├── SAMD00000345.jsonld
│       ├── SAMD00000345.ttl
│       ├── malformed_record.xml
│       ├── quotes.xml
│       └── with_blank_attrs.xml
└── scripts/
    └── daily_update.sh      # Cron wrapper
```

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `quick-xml` | Streaming XML pull parser |
| `flate2` | Gzip decompression |
| `clap` | CLI argument parsing |
| `serde` / `serde_json` | JSON-LD output + progress file |
| `md5` | Source file checksums |

TTL and N-Triples serializers are hand-rolled — the output format is fixed and simple, avoiding heavy RDF framework dependencies. JSON-LD is structured JSON via serde.

## Relationship to Other Projects

- **bsllmner-mk2**: Separate project that adds annotated/curated triples on top of the base RDF produced here. Uses the same IRI scheme (`idorg:{accession}`, `ddbj:{accession}#{property}`) to reference records and attributes.
- **RDF Portal**: Deposit target for the generated RDF data. Hosts a SPARQL endpoint for the dataset.
