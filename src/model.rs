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

use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};

/// Characters that must be percent-encoded in an IRI fragment.
/// Unreserved characters (- _ . ~) are intentionally excluded per RFC 3986.
const IRI_FRAGMENT_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'[')
    .add(b']')
    .add(b'{')
    .add(b'}')
    .add(b'|')
    .add(b'^')
    .add(b'`')
    .add(b'\\');

impl Attribute {
    /// Returns the preferred name for this attribute (harmonized_name > attribute_name).
    pub fn preferred_name(&self) -> &str {
        self.harmonized_name.as_deref().unwrap_or(&self.attribute_name)
    }

    /// Returns the property IRI fragment for this attribute, URL-encoded.
    /// Unreserved characters (- _ . ~) are preserved per RFC 3986.
    pub fn property_iri(&self, accession: &str) -> String {
        let name = self.preferred_name();
        let encoded = utf8_percent_encode(name, IRI_FRAGMENT_ENCODE_SET);
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
        // Underscore is an unreserved character (RFC 3986) and must NOT be encoded.
        // The harmonized_name "sample_name" should appear as-is in the IRI.
        assert_eq!(
            attr.property_iri("SAMN00000002"),
            "http://ddbj.nig.ac.jp/biosample/SAMN00000002#sample_name"
        );
    }

    #[test]
    fn test_attribute_property_iri_encodes_spaces_in_name() {
        let attr = Attribute {
            attribute_name: "sample name".to_string(),
            harmonized_name: None,
            display_name: None,
            value: None,
        };
        // Space must be encoded; underscore/hyphen must not.
        let iri = attr.property_iri("SAMN00000002");
        assert!(iri.contains("sample%20name"), "space should be encoded as %20, got: {}", iri);
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
