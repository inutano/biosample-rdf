use std::fs;
use std::io::BufReader;
use tempfile::TempDir;

use biosample_rdf::chunk::ChunkWriter;
use biosample_rdf::parser::BioSampleParser;
use biosample_rdf::progress::Progress;

const FIXTURE: &str = "tests/fixtures/biosample_set.xml.10";
const FIRST_ACCESSION: &str = "SAMN00000002";

/// Run the full pipeline (parser -> chunk writer) on the 10-record fixture and
/// return the temporary output directory. chunk_size is set larger than the
/// fixture so all records end up in a single chunk.
fn run_pipeline() -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    let out = dir.path();
    let progress = Progress::new(FIXTURE, 0, "test");
    let mut chunk_writer = ChunkWriter::new(out, 100, progress).expect("ChunkWriter");

    let file = fs::File::open(FIXTURE).expect("fixture file");
    let reader = BufReader::new(file);
    let mut parser = BioSampleParser::new(reader);

    loop {
        match parser.next_record() {
            Ok(Some(record)) => chunk_writer.add_record(record).expect("add_record"),
            Ok(None) => break,
            Err(e) => panic!("Parse error: {}", e),
        }
    }
    chunk_writer.finish().expect("finish");

    dir
}

#[test]
fn test_golden_10_records_ttl() {
    let dir = run_pipeline();
    let ttl_path = dir.path().join("ttl").join("chunk_0000.ttl");
    assert!(ttl_path.exists(), "TTL chunk file must exist");

    let content = fs::read_to_string(&ttl_path).expect("read TTL");

    // Prefixes present
    assert!(content.contains("@prefix idorg:"), "TTL should have idorg prefix");
    assert!(content.contains("@prefix ddbjont:"), "TTL should have ddbjont prefix");
    assert!(content.contains("@prefix dct:"), "TTL should have dct prefix");

    // First record present
    assert!(
        content.contains(&format!("idorg:{}", FIRST_ACCESSION)),
        "TTL should contain first record IRI"
    );
    assert!(
        content.contains("a ddbjont:BioSampleRecord"),
        "TTL should contain BioSampleRecord type"
    );
    assert!(
        content.contains(&format!("dct:identifier \"{}\"", FIRST_ACCESSION)),
        "TTL should contain first record identifier"
    );

    // Verify 10 records total by counting BioSampleRecord type assertions
    let count = content.matches("a ddbjont:BioSampleRecord").count();
    assert_eq!(count, 10, "TTL should have 10 BioSampleRecord instances, got {}", count);

    // Title for first record
    assert!(
        content.contains("Alistipes putredinis DSM 17216"),
        "TTL should contain first record title"
    );
}

#[test]
fn test_golden_10_records_ntriples() {
    let dir = run_pipeline();
    let nt_path = dir.path().join("nt").join("chunk_0000.nt");
    assert!(nt_path.exists(), "N-Triples chunk file must exist");

    let content = fs::read_to_string(&nt_path).expect("read N-Triples");

    // Every non-empty line ends with " ."
    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        assert!(
            trimmed.ends_with(" ."),
            "Line {} does not end with \" .\": {:?}",
            i + 1,
            trimmed
        );
    }

    // Count 10 BioSampleRecord type triples
    let count = content
        .lines()
        .filter(|l| {
            l.contains("BioSampleRecord") && l.contains("rdf-syntax-ns#type")
        })
        .count();
    assert_eq!(count, 10, "N-Triples should have 10 BioSampleRecord type triples, got {}", count);

    // First record IRI present
    let first_iri = format!("<http://identifiers.org/biosample/{}>", FIRST_ACCESSION);
    assert!(
        content.contains(&first_iri),
        "N-Triples should contain first record IRI: {}",
        first_iri
    );
}

#[test]
fn test_golden_10_records_jsonld() {
    let dir = run_pipeline();
    let jsonld_path = dir.path().join("jsonld").join("chunk_0000.jsonld");
    assert!(jsonld_path.exists(), "JSON-LD chunk file must exist");

    let content = fs::read_to_string(&jsonld_path).expect("read JSON-LD");

    // Parse as JSON array
    let parsed: serde_json::Value = serde_json::from_str(&content)
        .expect("JSON-LD chunk should be valid JSON");
    let array = parsed.as_array().expect("JSON-LD should be an array");

    // Verify 10 records
    assert_eq!(array.len(), 10, "JSON-LD should have 10 records, got {}", array.len());

    // First record has correct @id and dct:identifier
    let first = &array[0];
    let expected_id = format!("http://identifiers.org/biosample/{}", FIRST_ACCESSION);
    assert_eq!(
        first["@id"].as_str().unwrap_or(""),
        expected_id,
        "First record @id should be {}",
        expected_id
    );
    assert_eq!(
        first["dct:identifier"].as_str().unwrap_or(""),
        FIRST_ACCESSION,
        "First record dct:identifier should be {}",
        FIRST_ACCESSION
    );

    // All records have @id and dct:identifier
    for (i, record) in array.iter().enumerate() {
        assert!(
            record.get("@id").is_some(),
            "Record {} should have @id", i
        );
        let id = record["@id"].as_str().unwrap_or("");
        assert!(
            id.starts_with("http://identifiers.org/biosample/SAM"),
            "Record {} @id should start with biosample IRI, got: {}", i, id
        );

        assert!(
            record.get("dct:identifier").is_some(),
            "Record {} should have dct:identifier", i
        );
        let ident = record["dct:identifier"].as_str().unwrap_or("");
        assert!(
            ident.starts_with("SAM"),
            "Record {} dct:identifier should start with SAM, got: {}", i, ident
        );
    }
}
