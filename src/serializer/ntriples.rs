use std::io::Write;

use crate::model::BioSampleRecord;
use super::{
    Serializer,
    PREFIX_SCHEMA, PREFIX_IDORG, PREFIX_DCT, PREFIX_DDBJONT, PREFIX_RDFS, PREFIX_XSD,
};

#[derive(Debug, Clone, Default)]
pub struct NTriplesSerializer;

/// Escape a string for use as an N-Triples string literal.
pub fn escape_ntriples_string(s: &str) -> String {
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

impl Serializer for NTriplesSerializer {
    /// N-Triples has no header.
    fn write_header<W: Write>(&self, _writer: &mut W) -> std::io::Result<()> {
        Ok(())
    }

    fn write_record<W: Write>(&self, writer: &mut W, record: &BioSampleRecord) -> std::io::Result<()> {
        let acc = &record.accession;
        let subj = format!("{}{}", PREFIX_IDORG, acc);

        // rdf:type
        let rdf_type = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
        writeln!(
            writer,
            "<{}> <{}> <{}BioSampleRecord> .",
            subj, rdf_type, PREFIX_DDBJONT
        )?;

        // dct:identifier
        writeln!(
            writer,
            "<{}> <{}identifier> \"{}\" .",
            subj, PREFIX_DCT, escape_ntriples_string(acc)
        )?;

        // title -> dct:description + rdfs:label
        if let Some(title) = &record.title {
            writeln!(
                writer,
                "<{}> <{}description> \"{}\" .",
                subj, PREFIX_DCT, escape_ntriples_string(title)
            )?;
            writeln!(
                writer,
                "<{}> <{}label> \"{}\" .",
                subj, PREFIX_RDFS, escape_ntriples_string(title)
            )?;
        }

        // Dates
        let xsd_datetime = format!("{}dateTime", PREFIX_XSD);
        if let Some(date) = &record.submission_date {
            writeln!(
                writer,
                "<{}> <{}created> \"{}\"^^<{}> .",
                subj, PREFIX_DCT, escape_ntriples_string(date), xsd_datetime
            )?;
        }
        if let Some(date) = &record.last_update {
            writeln!(
                writer,
                "<{}> <{}modified> \"{}\"^^<{}> .",
                subj, PREFIX_DCT, escape_ntriples_string(date), xsd_datetime
            )?;
        }
        if let Some(date) = &record.publication_date {
            writeln!(
                writer,
                "<{}> <{}issued> \"{}\"^^<{}> .",
                subj, PREFIX_DCT, escape_ntriples_string(date), xsd_datetime
            )?;
        }

        // additionalProperty + PropertyValue nodes
        let schema_additional = format!("{}additionalProperty", PREFIX_SCHEMA);
        let schema_propval = format!("{}PropertyValue", PREFIX_SCHEMA);
        let schema_name = format!("{}name", PREFIX_SCHEMA);
        let schema_value = format!("{}value", PREFIX_SCHEMA);

        for attr in &record.attributes {
            let prop_iri = attr.property_iri(acc);
            let name = attr.preferred_name();
            let value = attr.value.as_deref().unwrap_or("");

            // subject -> additionalProperty -> prop_iri
            writeln!(
                writer,
                "<{}> <{}> <{}> .",
                subj, schema_additional, prop_iri
            )?;

            // prop_iri rdf:type PropertyValue
            writeln!(
                writer,
                "<{}> <{}> <{}> .",
                prop_iri, rdf_type, schema_propval
            )?;

            // prop_iri schema:name
            writeln!(
                writer,
                "<{}> <{}> \"{}\" .",
                prop_iri, schema_name, escape_ntriples_string(name)
            )?;

            // prop_iri schema:value
            writeln!(
                writer,
                "<{}> <{}> \"{}\" .",
                prop_iri, schema_value, escape_ntriples_string(value)
            )?;
        }

        Ok(())
    }

    /// N-Triples has no footer.
    fn write_footer<W: Write>(&self, _writer: &mut W) -> std::io::Result<()> {
        Ok(())
    }
}

impl NTriplesSerializer {
    pub fn new() -> Self {
        NTriplesSerializer
    }

    /// Write a complete record to a String (useful for testing).
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
    fn test_no_header_output() {
        let ser = NTriplesSerializer::new();
        let mut buf = Vec::new();
        ser.write_header(&mut buf).unwrap();
        assert!(buf.is_empty(), "N-Triples header should be empty");
    }

    #[test]
    fn test_every_line_ends_with_space_dot() {
        let ser = NTriplesSerializer::new();
        let s = ser.record_to_string(&sample_record());
        for line in s.lines() {
            assert!(
                line.ends_with(" ."),
                "Line does not end with \" .\": {:?}",
                line
            );
        }
    }

    #[test]
    fn test_contains_correct_full_iris() {
        let ser = NTriplesSerializer::new();
        let s = ser.record_to_string(&sample_record());
        assert!(s.contains("<http://identifiers.org/biosample/SAMD00000345>"));
        assert!(s.contains("<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>"));
        assert!(s.contains("<http://ddbj.nig.ac.jp/ontologies/biosample/BioSampleRecord>"));
        assert!(s.contains("<http://purl.org/dc/terms/identifier>"));
        assert!(s.contains("<http://purl.org/dc/terms/created>"));
        assert!(s.contains("<http://schema.org/additionalProperty>"));
        assert!(s.contains("<http://ddbj.nig.ac.jp/biosample/SAMD00000345#organism>"));
        assert!(s.contains("<http://schema.org/PropertyValue>"));
        assert!(s.contains("<http://schema.org/name>"));
        assert!(s.contains("<http://schema.org/value>"));
    }

    #[test]
    fn test_escape_ntriples_string() {
        assert_eq!(escape_ntriples_string("say \"hi\""), "say \\\"hi\\\"");
        assert_eq!(escape_ntriples_string("line1\nline2"), "line1\\nline2");
        assert_eq!(escape_ntriples_string("back\\slash"), "back\\\\slash");
    }

    #[test]
    fn test_no_prefixed_names_in_output() {
        let ser = NTriplesSerializer::new();
        let s = ser.record_to_string(&sample_record());
        // Must not contain Turtle-style prefixed names
        assert!(!s.contains("idorg:"));
        assert!(!s.contains("dct:"));
        assert!(!s.contains("ddbjont:"));
        assert!(!s.contains("rdfs:"));
        assert!(!s.contains("xsd:"));
    }

    #[test]
    fn test_empty_attributes() {
        let ser = NTriplesSerializer::new();
        let rec = BioSampleRecord {
            accession: "SAMD00000001".to_string(),
            submission_date: None,
            last_update: None,
            publication_date: None,
            title: None,
            attributes: vec![],
        };
        let s = ser.record_to_string(&rec);
        for line in s.lines() {
            assert!(line.ends_with(" ."), "Line: {:?}", line);
        }
        assert!(!s.contains("additionalProperty"));
    }
}
