use std::io::Write;

use crate::model::BioSampleRecord;
use super::{
    Serializer,
    PREFIX_SCHEMA, PREFIX_IDORG, PREFIX_DCT, PREFIX_DDBJONT, PREFIX_RDFS, PREFIX_XSD,
};

#[derive(Debug, Clone, Default)]
pub struct TurtleSerializer;

/// Escape a string for use as a Turtle string literal.
pub fn escape_turtle_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"'  => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c    => out.push(c),
        }
    }
    out
}

impl Serializer for TurtleSerializer {
    fn write_header<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writeln!(writer, "@prefix : <{}> .", PREFIX_SCHEMA)?;
        writeln!(writer, "@prefix idorg: <{}> .", PREFIX_IDORG)?;
        writeln!(writer, "@prefix dct: <{}> .", PREFIX_DCT)?;
        writeln!(writer, "@prefix ddbjont: <{}> .", PREFIX_DDBJONT)?;
        writeln!(writer, "@prefix rdfs: <{}> .", PREFIX_RDFS)?;
        writeln!(writer, "@prefix xsd: <{}> .", PREFIX_XSD)?;
        writeln!(writer)?;
        Ok(())
    }

    fn write_record<W: Write>(&self, writer: &mut W, record: &BioSampleRecord) -> std::io::Result<()> {
        let acc = &record.accession;

        // Collect predicate-object lines for the main subject block.
        // Each entry is the text after the two-space indent, WITHOUT the trailing ; or .
        let mut po_lines: Vec<String> = Vec::new();

        po_lines.push("a ddbjont:BioSampleRecord".to_string());
        po_lines.push(format!("dct:identifier \"{}\"", escape_turtle_string(acc)));

        if let Some(title) = &record.title {
            po_lines.push(format!("dct:description \"{}\"", escape_turtle_string(title)));
            po_lines.push(format!("rdfs:label \"{}\"", escape_turtle_string(title)));
        }

        if let Some(date) = &record.submission_date {
            po_lines.push(format!(
                "dct:created \"{}\"^^xsd:dateTime",
                escape_turtle_string(date)
            ));
        }
        if let Some(date) = &record.last_update {
            po_lines.push(format!(
                "dct:modified \"{}\"^^xsd:dateTime",
                escape_turtle_string(date)
            ));
        }
        if let Some(date) = &record.publication_date {
            po_lines.push(format!(
                "dct:issued \"{}\"^^xsd:dateTime",
                escape_turtle_string(date)
            ));
        }

        // additionalProperty references — each attribute gets its own :additionalProperty line
        // so we represent them as multi-value using commas then dot on the last.
        // We'll handle the attribute block as two sub-lines per attribute in po_lines approach;
        // instead let's build the additionalProperty block as a single po entry string.
        if !record.attributes.is_empty() {
            let prop_iris: Vec<String> = record
                .attributes
                .iter()
                .map(|a| format!("    <{}>", a.property_iri(acc)))
                .collect();
            let ap_block = format!("  :additionalProperty\n{}", prop_iris.join(" ,\n"));
            // This one we write specially at the end with " ."
            // Write the main subject line
            writeln!(writer, "idorg:{}", acc)?;
            for line in po_lines.iter() {
                writeln!(writer, "  {} ;", line)?;
            }
            // Write additionalProperty block with " ."
            writeln!(writer, "{} .", ap_block)?;
        } else {
            // No attributes: write subject block, last line ends with "."
            writeln!(writer, "idorg:{}", acc)?;
            let last = po_lines.len().saturating_sub(1);
            for (i, line) in po_lines.iter().enumerate() {
                if i < last {
                    writeln!(writer, "  {} ;", line)?;
                } else {
                    writeln!(writer, "  {} .", line)?;
                }
            }
        }
        writeln!(writer)?;

        // Property value nodes
        for attr in &record.attributes {
            let prop_iri = attr.property_iri(acc);
            let name = attr.preferred_name();
            let value = attr.value.as_deref().unwrap_or("");
            writeln!(writer, "<{}> a :PropertyValue ;", prop_iri)?;
            writeln!(writer, "  :name \"{}\" ;", escape_turtle_string(name))?;
            writeln!(writer, "  :value \"{}\" .", escape_turtle_string(value))?;
            writeln!(writer)?;
        }

        Ok(())
    }

    fn write_footer<W: Write>(&self, _writer: &mut W) -> std::io::Result<()> {
        Ok(())
    }
}

impl TurtleSerializer {
    pub fn new() -> Self {
        TurtleSerializer
    }

    /// Write a complete record to a String (useful for testing and single-record use).
    pub fn record_to_string(&self, record: &BioSampleRecord) -> String {
        let mut buf = Vec::new();
        self.write_record(&mut buf, record).unwrap();
        String::from_utf8(buf).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BioSampleRecord, Attribute};

    fn sample_record() -> BioSampleRecord {
        BioSampleRecord {
            accession: "SAMD00000345".to_string(),
            submission_date: Some("2014-07-30T00:00:00Z".to_string()),
            last_update: None,
            publication_date: None,
            title: Some("type strain".to_string()),
            attributes: vec![
                Attribute {
                    attribute_name: "organism".to_string(),
                    harmonized_name: Some("organism".to_string()),
                    display_name: None,
                    value: Some("Homo sapiens".to_string()),
                },
            ],
        }
    }

    #[test]
    fn test_header_contains_prefixes() {
        let ser = TurtleSerializer::new();
        let mut buf = Vec::new();
        ser.write_header(&mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("@prefix : <http://schema.org/> ."));
        assert!(s.contains("@prefix idorg: <http://identifiers.org/biosample/> ."));
        assert!(s.contains("@prefix dct: <http://purl.org/dc/terms/> ."));
        assert!(s.contains("@prefix ddbjont: <http://ddbj.nig.ac.jp/ontologies/biosample/> ."));
        assert!(s.contains("@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> ."));
        assert!(s.contains("@prefix xsd: <http://www.w3.org/2001/XMLSchema#> ."));
    }

    #[test]
    fn test_record_output_contains_correct_triples() {
        let ser = TurtleSerializer::new();
        let rec = sample_record();
        let s = ser.record_to_string(&rec);
        assert!(s.contains("idorg:SAMD00000345"));
        assert!(s.contains("a ddbjont:BioSampleRecord"));
        assert!(s.contains("dct:identifier \"SAMD00000345\""));
        assert!(s.contains("dct:description \"type strain\""));
        assert!(s.contains("rdfs:label \"type strain\""));
        assert!(s.contains("dct:created \"2014-07-30T00:00:00Z\"^^xsd:dateTime"));
        assert!(s.contains(":additionalProperty"));
        assert!(s.contains("http://ddbj.nig.ac.jp/biosample/SAMD00000345#organism"));
        assert!(s.contains("a :PropertyValue"));
        assert!(s.contains(":name \"organism\""));
        assert!(s.contains(":value \"Homo sapiens\""));
    }

    #[test]
    fn test_special_characters_are_escaped() {
        assert_eq!(escape_turtle_string("say \"hi\""), "say \\\"hi\\\"");
        assert_eq!(escape_turtle_string("line1\nline2"), "line1\\nline2");
        assert_eq!(escape_turtle_string("tab\there"), "tab\\there");
        assert_eq!(escape_turtle_string("back\\slash"), "back\\\\slash");
        assert_eq!(escape_turtle_string("cr\rhere"), "cr\\rhere");
    }

    #[test]
    fn test_empty_attributes_list() {
        let ser = TurtleSerializer::new();
        let rec = BioSampleRecord {
            accession: "SAMD00000001".to_string(),
            submission_date: None,
            last_update: None,
            publication_date: None,
            title: None,
            attributes: vec![],
        };
        let s = ser.record_to_string(&rec);
        assert!(s.contains("idorg:SAMD00000001"));
        assert!(!s.contains(":additionalProperty"));
        assert!(!s.contains(":PropertyValue"));
        // Must end with "." not ";"
        let trimmed = s.trim();
        assert!(trimmed.ends_with('.'), "Record should end with '.' but got: {}", trimmed);
    }

    #[test]
    fn test_attribute_with_none_value_emits_empty_string() {
        let ser = TurtleSerializer::new();
        let rec = BioSampleRecord {
            accession: "SAMD00000002".to_string(),
            submission_date: None,
            last_update: None,
            publication_date: None,
            title: None,
            attributes: vec![
                Attribute {
                    attribute_name: "missing_val".to_string(),
                    harmonized_name: None,
                    display_name: None,
                    value: None,
                },
            ],
        };
        let s = ser.record_to_string(&rec);
        assert!(s.contains(":value \"\""));
    }
}
