# biosample-rdf

Convert [NCBI BioSample](https://www.ncbi.nlm.nih.gov/biosample/) XML dumps to RDF. Streams the full `biosample_set.xml.gz` (~4 GB compressed, 53M+ records) through a single-pass chunked pipeline, producing **Turtle**, **JSON-LD**, and **N-Triples** output.

## Install

```bash
cargo install --path .
```

Or with Docker:

```bash
docker build -t biosample-rdf .
```

## Usage

### Convert

```bash
biosample-rdf convert \
  --input biosample_set.xml.gz \
  --output-dir ./output \
  --chunk-size 100000
```

Options:

| Flag | Default | Description |
|------|---------|-------------|
| `-i, --input` | (required) | Path to `biosample_set.xml(.gz)` |
| `-o, --output-dir` | `./output` | Output directory |
| `-c, --chunk-size` | `100000` | Records per output chunk |

Output structure:

```
output/
  ttl/chunk_0000.ttl ... chunk_NNNN.ttl
  jsonld/chunk_0000.jsonld ... chunk_NNNN.jsonld
  nt/chunk_0000.nt ... chunk_NNNN.nt
  manifest.json
  progress.json
  errors.log
```

### Validate

```bash
biosample-rdf validate ./output
```

Checks all `.ttl` and `.nt` files for structural correctness (well-formed triples, valid IRIs, no empty identifiers).

### Daily update script

`scripts/daily_update.sh` orchestrates download, convert, validate, and optional webhook notification. See the script for environment variables.

```bash
DATA_DIR=/data CHUNK_SIZE=100000 bash scripts/daily_update.sh
```

## RDF schema

Each BioSample record maps to:

```turtle
@prefix idorg: <http://identifiers.org/biosample/> .
@prefix dct:   <http://purl.org/dc/terms/> .
@prefix ddbjont: <http://ddbj.nig.ac.jp/ontologies/biosample/> .
@prefix :      <http://schema.org/> .

idorg:SAMN00000002
  a ddbjont:BioSampleRecord ;
  dct:identifier "SAMN00000002" ;
  dct:description "Alistipes putredinis DSM 17216" ;
  rdfs:label "Alistipes putredinis DSM 17216" ;
  dct:created "2008-04-04T08:44:24.950"^^xsd:dateTime ;
  dct:modified "2022-09-25T02:00:02.729"^^xsd:dateTime ;
  :additionalProperty
    <http://ddbj.nig.ac.jp/biosample/SAMN00000002#organism> .

<http://ddbj.nig.ac.jp/biosample/SAMN00000002#organism>
  a :PropertyValue ;
  :name "organism" ;
  :value "Alistipes putredinis DSM 17216" .
```

- Record IRI: `http://identifiers.org/biosample/{accession}`
- Attribute IRI: `http://ddbj.nig.ac.jp/biosample/{accession}#{property}`
- Uses `harmonized_name` when available, falls back to `attribute_name`

## Benchmark

Full NCBI BioSample dump (`biosample_set.xml.gz`, 2026-04-01 snapshot):

| Metric | Value |
|--------|-------|
| Input size | 4.0 GB (gzip compressed) |
| Total records | 53,342,722 |
| Records skipped | 0 |
| Output chunks | 534 |
| Output size (Turtle) | 174 GB |
| Output size (JSON-LD) | 183 GB |
| Output size (N-Triples) | 429 GB |
| **Total output** | **785 GB** |
| Conversion time | 55 min |
| Throughput | ~16,200 records/sec |
| Peak memory | ~966 MB |
| Validation | 1,068 files, 0 errors |

Hardware: Intel Xeon w5-3435X, 128 GB RAM, NVMe storage.

## License

MIT
