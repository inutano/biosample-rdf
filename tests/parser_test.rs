use std::fs::File;
use std::io::BufReader;

#[test]
fn test_parse_10_record_fixture() {
    let file = File::open("tests/fixtures/biosample_set.xml.10").expect("fixture file not found");
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
    assert_eq!(records.len(), 10, "Expected 10 records, got {}", records.len());
    assert_eq!(errors.len(), 0, "Expected no errors, got {:?}", errors);
    assert_eq!(records[0].accession, "SAMN00000002");
    assert_eq!(records[0].title.as_deref(), Some("Alistipes putredinis DSM 17216"));
    assert!(records[0].attributes.len() > 10);
    assert!(!records[9].attributes.is_empty());
}

#[test]
fn test_parse_quotes_fixture() {
    let file = File::open("tests/fixtures/quotes.xml").expect("fixture file not found");
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
    let public_attr = records[0].attributes.iter()
        .find(|a| a.attribute_name.contains("PUBLIC"))
        .expect("Should find PUBLIC attribute");
    assert_eq!(public_attr.attribute_name, "\"PUBLIC\"");
}

#[test]
fn test_parse_blank_attrs_fixture() {
    let file = File::open("tests/fixtures/with_blank_attrs.xml").expect("fixture file not found");
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
    let empty_attrs: Vec<_> = records[0].attributes.iter()
        .filter(|a| a.value.is_none())
        .collect();
    assert!(empty_attrs.len() >= 5, "Expected at least 5 empty attributes, got {}", empty_attrs.len());
}

#[test]
fn test_parse_malformed_fixture() {
    let file = File::open("tests/fixtures/malformed_record.xml").expect("fixture file not found");
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
    assert_eq!(records.len(), 2, "Expected 2 valid records");
    assert_eq!(errors.len(), 1, "Expected 1 error for missing accession");
    assert_eq!(records[0].accession, "SAMN99999999");
    assert_eq!(records[1].accession, "SAMN99999997");
}
