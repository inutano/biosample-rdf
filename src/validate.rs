use std::fs;
use std::path::{Path, PathBuf};

/// Result of validating a single RDF file.
#[derive(Debug)]
pub struct ValidationResult {
    pub file: String,
    pub errors: Vec<String>,
    pub record_count: usize,
}

/// Validate a Turtle (.ttl) file for structural correctness.
pub fn validate_turtle(path: &Path) -> ValidationResult {
    let file = path.display().to_string();
    let mut errors = Vec::new();
    let mut record_count = 0;

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            return ValidationResult {
                file,
                errors: vec![format!("Could not read file: {}", e)],
                record_count: 0,
            };
        }
    };

    for line in content.lines() {
        // Count BioSampleRecord instances
        if line.contains("a ddbjont:BioSampleRecord") {
            record_count += 1;
        }

        // Check for empty identifiers
        if line.contains("dct:identifier \"\"") {
            errors.push(format!("Empty dct:identifier found: {}", line.trim()));
        }

        // Check for malformed record IRIs (idorg: that doesn't start with SAM)
        if let Some(idx) = line.find("idorg:") {
            let after = &line[idx + 6..];
            // Extract the local name (up to whitespace or end of significant chars)
            let local: String = after
                .chars()
                .take_while(|c| !c.is_whitespace() && *c != ';' && *c != '.' && *c != '\n')
                .collect();
            if !local.is_empty() && !local.starts_with("SAM") {
                errors.push(format!(
                    "Malformed record IRI (idorg:{}) does not start with SAM: {}",
                    local,
                    line.trim()
                ));
            }
        }
    }

    ValidationResult {
        file,
        errors,
        record_count,
    }
}

/// Validate an N-Triples (.nt) file for structural correctness.
pub fn validate_ntriples(path: &Path) -> ValidationResult {
    let file = path.display().to_string();
    let mut errors = Vec::new();
    let mut record_count = 0;

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            return ValidationResult {
                file,
                errors: vec![format!("Could not read file: {}", e)],
                record_count: 0,
            };
        }
    };

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Every non-empty, non-comment line must end with " ."
        if !trimmed.ends_with(" .") {
            errors.push(format!(
                "Line {} does not end with \" .\": {}",
                line_num + 1,
                trimmed
            ));
        }

        // Count BioSampleRecord type triples
        if trimmed.contains("BioSampleRecord")
            && trimmed.contains("rdf-syntax-ns#type")
        {
            record_count += 1;
        }
    }

    ValidationResult {
        file,
        errors,
        record_count,
    }
}

/// Walk a directory and validate all .ttl and .nt files found.
pub fn validate_directory(dir: &Path) -> Vec<ValidationResult> {
    let mut results = Vec::new();

    // Validate ttl/ subdirectory
    let ttl_dir = dir.join("ttl");
    if ttl_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&ttl_dir) {
            let mut paths: Vec<PathBuf> = entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().map(|e| e == "ttl").unwrap_or(false))
                .collect();
            paths.sort();
            for path in paths {
                results.push(validate_turtle(&path));
            }
        }
    }

    // Validate nt/ subdirectory
    let nt_dir = dir.join("nt");
    if nt_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&nt_dir) {
            let mut paths: Vec<PathBuf> = entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().map(|e| e == "nt").unwrap_or(false))
                .collect();
            paths.sort();
            for path in paths {
                results.push(validate_ntriples(&path));
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    use tempfile::TempDir;

    fn write_temp_ttl(content: &str) -> NamedTempFile {
        let mut f = tempfile::Builder::new()
            .suffix(".ttl")
            .tempfile()
            .unwrap();
        write!(f, "{}", content).unwrap();
        f
    }

    fn write_temp_nt(content: &str) -> NamedTempFile {
        let mut f = tempfile::Builder::new()
            .suffix(".nt")
            .tempfile()
            .unwrap();
        write!(f, "{}", content).unwrap();
        f
    }

    const VALID_TTL: &str = r#"@prefix idorg: <http://identifiers.org/biosample/> .
@prefix dct: <http://purl.org/dc/terms/> .
@prefix ddbjont: <http://ddbj.nig.ac.jp/ontologies/biosample/> .

idorg:SAMN00000002
  a ddbjont:BioSampleRecord ;
  dct:identifier "SAMN00000002" .

idorg:SAMN00000003
  a ddbjont:BioSampleRecord ;
  dct:identifier "SAMN00000003" .
"#;

    const EMPTY_IDENTIFIER_TTL: &str = r#"@prefix idorg: <http://identifiers.org/biosample/> .
@prefix dct: <http://purl.org/dc/terms/> .
@prefix ddbjont: <http://ddbj.nig.ac.jp/ontologies/biosample/> .

idorg:SAMN00000002
  a ddbjont:BioSampleRecord ;
  dct:identifier "" .
"#;

    const VALID_NT: &str = "<http://identifiers.org/biosample/SAMN00000002> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://ddbj.nig.ac.jp/ontologies/biosample/BioSampleRecord> .\n<http://identifiers.org/biosample/SAMN00000002> <http://purl.org/dc/terms/identifier> \"SAMN00000002\" .\n<http://identifiers.org/biosample/SAMN00000003> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://ddbj.nig.ac.jp/ontologies/biosample/BioSampleRecord> .\n<http://identifiers.org/biosample/SAMN00000003> <http://purl.org/dc/terms/identifier> \"SAMN00000003\" .\n";

    #[test]
    fn test_valid_ttl_no_errors() {
        let f = write_temp_ttl(VALID_TTL);
        let result = validate_turtle(f.path());
        assert!(result.errors.is_empty(), "Expected no errors, got: {:?}", result.errors);
        assert_eq!(result.record_count, 2);
    }

    #[test]
    fn test_ttl_empty_identifier_detected() {
        let f = write_temp_ttl(EMPTY_IDENTIFIER_TTL);
        let result = validate_turtle(f.path());
        assert!(!result.errors.is_empty(), "Expected errors for empty identifier");
        assert!(result.errors.iter().any(|e| e.contains("Empty dct:identifier")));
    }

    #[test]
    fn test_ttl_malformed_iri_detected() {
        let bad_ttl = r#"@prefix idorg: <http://identifiers.org/biosample/> .
@prefix dct: <http://purl.org/dc/terms/> .
@prefix ddbjont: <http://ddbj.nig.ac.jp/ontologies/biosample/> .

idorg:BADRECORD123
  a ddbjont:BioSampleRecord ;
  dct:identifier "BADRECORD123" .
"#;
        let f = write_temp_ttl(bad_ttl);
        let result = validate_turtle(f.path());
        assert!(
            result.errors.iter().any(|e| e.contains("does not start with SAM")),
            "Expected malformed IRI error, got: {:?}", result.errors
        );
    }

    #[test]
    fn test_valid_ntriples_no_errors() {
        let f = write_temp_nt(VALID_NT);
        let result = validate_ntriples(f.path());
        assert!(result.errors.is_empty(), "Expected no errors, got: {:?}", result.errors);
        assert_eq!(result.record_count, 2);
    }

    #[test]
    fn test_ntriples_missing_dot_detected() {
        let bad_nt = "<http://identifiers.org/biosample/SAMN00000002> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://ddbj.nig.ac.jp/ontologies/biosample/BioSampleRecord>\n";
        let f = write_temp_nt(bad_nt);
        let result = validate_ntriples(f.path());
        assert!(!result.errors.is_empty(), "Expected errors for missing \" .\"");
        assert!(result.errors.iter().any(|e| e.contains("does not end with")));
    }

    #[test]
    fn test_validate_directory_finds_files() {
        let dir = TempDir::new().unwrap();
        let ttl_dir = dir.path().join("ttl");
        let nt_dir = dir.path().join("nt");
        fs::create_dir_all(&ttl_dir).unwrap();
        fs::create_dir_all(&nt_dir).unwrap();

        // Write a valid TTL file
        let ttl_path = ttl_dir.join("chunk_0000.ttl");
        fs::write(&ttl_path, VALID_TTL).unwrap();

        // Write a valid NT file
        let nt_path = nt_dir.join("chunk_0000.nt");
        fs::write(&nt_path, VALID_NT).unwrap();

        let results = validate_directory(dir.path());
        assert_eq!(results.len(), 2);
        for result in &results {
            assert!(result.errors.is_empty(), "Expected no errors in {}: {:?}", result.file, result.errors);
        }
        let total_records: usize = results.iter().map(|r| r.record_count).sum();
        assert_eq!(total_records, 4); // 2 from TTL + 2 from NT
    }
}
