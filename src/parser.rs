use std::io::BufRead;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::error::BsError;
use crate::model::{Attribute, BioSampleRecord};

/// A streaming parser for NCBI BioSample XML.
///
/// Reads one `<BioSample>` element at a time from the underlying reader.
/// Call `next_record()` in a loop until it returns `Ok(None)` (end of input).
/// If a record is malformed (e.g. missing accession), it returns an `Err`
/// and the caller can log the error and continue calling `next_record()`.
pub struct BioSampleParser<R: BufRead> {
    reader: Reader<R>,
    buf: Vec<u8>,
}

impl<R: BufRead> BioSampleParser<R> {
    pub fn new(reader: R) -> Self {
        let mut xml_reader = Reader::from_reader(reader);
        xml_reader.config_mut().trim_text(true);
        BioSampleParser {
            reader: xml_reader,
            buf: Vec::new(),
        }
    }

    /// Parse the next `<BioSample>` record from the stream.
    ///
    /// Returns:
    /// - `Ok(Some(record))` for a successfully parsed record
    /// - `Ok(None)` at end of input
    /// - `Err(BsError::MissingAccession)` if the record lacks an accession attribute
    /// - `Err(BsError::XmlParse)` on XML parsing errors
    pub fn next_record(&mut self) -> Result<Option<BioSampleRecord>, BsError> {
        // Scan forward until we find a <BioSample ...> start tag
        loop {
            self.buf.clear();
            let event = self.reader.read_event_into(&mut self.buf).map_err(|e| {
                BsError::XmlParse {
                    offset: self.reader.error_position(),
                    message: e.to_string(),
                }
            })?;

            match event {
                Event::Start(ref e) if e.name().as_ref() == b"BioSample" => {
                    // Extract attributes from the <BioSample> start tag before moving on
                    let offset = self.reader.buffer_position();
                    let mut accession: Option<String> = None;
                    let mut submission_date: Option<String> = None;
                    let mut last_update: Option<String> = None;
                    let mut publication_date: Option<String> = None;

                    for attr_result in e.attributes() {
                        let attr = attr_result.map_err(|err| BsError::XmlParse {
                            offset,
                            message: format!("attribute parse error: {}", err),
                        })?;
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = attr
                            .unescape_value()
                            .map_err(|err| BsError::XmlParse {
                                offset,
                                message: format!("attribute unescape error: {}", err),
                            })?
                            .into_owned();

                        match key {
                            "accession" => accession = Some(val),
                            "submission_date" => submission_date = Some(val),
                            "last_update" => last_update = Some(val),
                            "publication_date" => publication_date = Some(val),
                            _ => {}
                        }
                    }

                    // Now parse the children
                    let (title, attributes) = self.parse_biosample_children()?;

                    return match accession {
                        Some(acc) => Ok(Some(BioSampleRecord {
                            accession: acc,
                            submission_date,
                            last_update,
                            publication_date,
                            title,
                            attributes,
                        })),
                        None => Err(BsError::MissingAccession { offset }),
                    };
                }
                Event::Eof => return Ok(None),
                _ => {
                    // Skip everything else (BioSampleSet wrapper, whitespace, etc.)
                }
            }
        }
    }

    /// Parse the children of a `<BioSample>` element until its closing tag.
    /// Returns (title, attributes).
    fn parse_biosample_children(&mut self) -> Result<(Option<String>, Vec<Attribute>), BsError> {
        let mut title: Option<String> = None;
        let mut attributes: Vec<Attribute> = Vec::new();
        let mut depth: u32 = 1; // we are inside <BioSample>
        let mut in_title = false;
        let mut in_attribute = false;
        let mut current_attr_name = String::new();
        let mut current_attr_harmonized: Option<String> = None;
        let mut current_attr_display: Option<String> = None;
        let mut current_attr_text = String::new();
        let mut title_text = String::new();

        loop {
            self.buf.clear();
            let event = self.reader.read_event_into(&mut self.buf).map_err(|e| {
                BsError::XmlParse {
                    offset: self.reader.error_position(),
                    message: e.to_string(),
                }
            })?;

            match event {
                Event::Start(ref e) => {
                    let name = e.name();
                    let tag = name.as_ref();

                    if depth == 1 && tag == b"Description" {
                        depth += 1;
                    } else if depth == 2 && tag == b"Title" {
                        in_title = true;
                        title_text.clear();
                        depth += 1;
                    } else if depth == 1 && tag == b"Attributes" {
                        depth += 1;
                    } else if depth == 2 && tag == b"Attribute" {
                        in_attribute = true;
                        current_attr_text.clear();
                        current_attr_name.clear();
                        current_attr_harmonized = None;
                        current_attr_display = None;

                        let offset = self.reader.buffer_position();
                        for attr_result in e.attributes() {
                            let attr = attr_result.map_err(|err| BsError::XmlParse {
                                offset,
                                message: format!("attribute parse error: {}", err),
                            })?;
                            let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                            let val = attr
                                .unescape_value()
                                .map_err(|err| BsError::XmlParse {
                                    offset,
                                    message: format!("attribute unescape error: {}", err),
                                })?
                                .into_owned();

                            match key {
                                "attribute_name" => current_attr_name = val,
                                "harmonized_name" => current_attr_harmonized = Some(val),
                                "display_name" => current_attr_display = Some(val),
                                _ => {}
                            }
                        }
                        depth += 1;
                    } else {
                        depth += 1;
                    }
                }
                Event::End(ref e) => {
                    let name = e.name();
                    let tag = name.as_ref();

                    if tag == b"BioSample" && depth == 1 {
                        break;
                    }

                    if in_title && tag == b"Title" {
                        in_title = false;
                        if !title_text.is_empty() {
                            title = Some(title_text.clone());
                        }
                    }

                    if in_attribute && tag == b"Attribute" {
                        let value = if current_attr_text.is_empty() {
                            None
                        } else {
                            Some(current_attr_text.clone())
                        };
                        attributes.push(Attribute {
                            attribute_name: current_attr_name.clone(),
                            harmonized_name: current_attr_harmonized.take(),
                            display_name: current_attr_display.take(),
                            value,
                        });
                        in_attribute = false;
                        current_attr_text.clear();
                    }

                    depth -= 1;
                }
                Event::Empty(ref e) => {
                    let name = e.name();
                    let tag = name.as_ref();

                    if depth == 2 && tag == b"Attribute" {
                        // Self-closing <Attribute .../> -- value is None
                        let mut attr_name = String::new();
                        let mut harmonized: Option<String> = None;
                        let mut display: Option<String> = None;

                        let offset = self.reader.buffer_position();
                        for attr_result in e.attributes() {
                            let attr = attr_result.map_err(|err| BsError::XmlParse {
                                offset,
                                message: format!("attribute parse error: {}", err),
                            })?;
                            let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                            let val = attr
                                .unescape_value()
                                .map_err(|err| BsError::XmlParse {
                                    offset,
                                    message: format!("attribute unescape error: {}", err),
                                })?
                                .into_owned();

                            match key {
                                "attribute_name" => attr_name = val,
                                "harmonized_name" => harmonized = Some(val),
                                "display_name" => display = Some(val),
                                _ => {}
                            }
                        }

                        attributes.push(Attribute {
                            attribute_name: attr_name,
                            harmonized_name: harmonized,
                            display_name: display,
                            value: None,
                        });
                    }
                    // Other self-closing tags (Organism, Contact, Attributes, etc.) -- ignore
                }
                Event::Text(ref e) => {
                    if in_title {
                        let text = e.unescape().map_err(|err| BsError::XmlParse {
                            offset: self.reader.buffer_position(),
                            message: format!("text unescape error: {}", err),
                        })?;
                        title_text.push_str(&text);
                    } else if in_attribute {
                        let text = e.unescape().map_err(|err| BsError::XmlParse {
                            offset: self.reader.buffer_position(),
                            message: format!("text unescape error: {}", err),
                        })?;
                        current_attr_text.push_str(&text);
                    }
                }
                Event::Eof => {
                    // Unexpected EOF inside a record
                    break;
                }
                _ => {}
            }
        }

        Ok((title, attributes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn parse_all(xml: &str) -> (Vec<BioSampleRecord>, Vec<BsError>) {
        let cursor = Cursor::new(xml.as_bytes().to_vec());
        let reader = std::io::BufReader::new(cursor);
        let mut parser = BioSampleParser::new(reader);
        let mut records = Vec::new();
        let mut errors = Vec::new();
        loop {
            match parser.next_record() {
                Ok(Some(rec)) => records.push(rec),
                Ok(None) => break,
                Err(e) => errors.push(e),
            }
        }
        (records, errors)
    }

    #[test]
    fn test_single_record() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<BioSampleSet>
<BioSample submission_date="2020-01-15" last_update="2021-06-01" publication_date="2020-02-01" access="public" id="123" accession="SAMN12345678">
  <Ids>
    <Id db="BioSample" is_primary="1">SAMN12345678</Id>
  </Ids>
  <Description>
    <Title>Test organism sample</Title>
  </Description>
  <Attributes>
    <Attribute attribute_name="organism" harmonized_name="organism" display_name="organism">Homo sapiens</Attribute>
    <Attribute attribute_name="strain" harmonized_name="strain">K-12</Attribute>
  </Attributes>
</BioSample>
</BioSampleSet>"#;

        let (records, errors) = parse_all(xml);
        assert_eq!(errors.len(), 0);
        assert_eq!(records.len(), 1);
        let rec = &records[0];
        assert_eq!(rec.accession, "SAMN12345678");
        assert_eq!(rec.submission_date.as_deref(), Some("2020-01-15"));
        assert_eq!(rec.last_update.as_deref(), Some("2021-06-01"));
        assert_eq!(rec.publication_date.as_deref(), Some("2020-02-01"));
        assert_eq!(rec.title.as_deref(), Some("Test organism sample"));
        assert_eq!(rec.attributes.len(), 2);
        assert_eq!(rec.attributes[0].attribute_name, "organism");
        assert_eq!(rec.attributes[0].harmonized_name.as_deref(), Some("organism"));
        assert_eq!(rec.attributes[0].display_name.as_deref(), Some("organism"));
        assert_eq!(rec.attributes[0].value.as_deref(), Some("Homo sapiens"));
        assert_eq!(rec.attributes[1].attribute_name, "strain");
        assert_eq!(rec.attributes[1].display_name, None);
        assert_eq!(rec.attributes[1].value.as_deref(), Some("K-12"));
    }

    #[test]
    fn test_empty_attribute() {
        let xml = r#"<BioSampleSet>
<BioSample accession="SAMN00001111" submission_date="2020-01-01">
  <Description><Title>Empty attr test</Title></Description>
  <Attributes>
    <Attribute attribute_name="exp_ammonium"></Attribute>
    <Attribute attribute_name="organism">E. coli</Attribute>
  </Attributes>
</BioSample>
</BioSampleSet>"#;

        let (records, errors) = parse_all(xml);
        assert_eq!(errors.len(), 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].attributes.len(), 2);
        assert_eq!(records[0].attributes[0].value, None);
        assert_eq!(records[0].attributes[1].value.as_deref(), Some("E. coli"));
    }

    #[test]
    fn test_self_closing_attribute() {
        let xml = r#"<BioSampleSet>
<BioSample accession="SAMN00002222" submission_date="2020-01-01">
  <Description><Title>Self-closing test</Title></Description>
  <Attributes>
    <Attribute attribute_name="missing_data"/>
    <Attribute attribute_name="organism">Test</Attribute>
  </Attributes>
</BioSample>
</BioSampleSet>"#;

        let (records, errors) = parse_all(xml);
        assert_eq!(errors.len(), 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].attributes.len(), 2);
        assert_eq!(records[0].attributes[0].attribute_name, "missing_data");
        assert_eq!(records[0].attributes[0].value, None);
    }

    #[test]
    fn test_missing_accession() {
        let xml = r#"<BioSampleSet>
<BioSample submission_date="2020-01-01" access="public" id="999">
  <Ids><Id db="BioSample">BROKEN</Id></Ids>
  <Description><Title>No accession</Title></Description>
  <Attributes>
    <Attribute attribute_name="organism">Test</Attribute>
  </Attributes>
</BioSample>
</BioSampleSet>"#;

        let (records, errors) = parse_all(xml);
        assert_eq!(records.len(), 0);
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], BsError::MissingAccession { .. }));
    }

    #[test]
    fn test_continues_after_bad_record() {
        let xml = r#"<BioSampleSet>
<BioSample accession="SAMN11111111" submission_date="2020-01-01">
  <Description><Title>Good one</Title></Description>
  <Attributes><Attribute attribute_name="organism">A</Attribute></Attributes>
</BioSample>
<BioSample submission_date="2020-01-01" id="bad">
  <Description><Title>Missing accession</Title></Description>
  <Attributes><Attribute attribute_name="organism">B</Attribute></Attributes>
</BioSample>
<BioSample accession="SAMN33333333" submission_date="2020-01-01">
  <Description><Title>Also good</Title></Description>
  <Attributes><Attribute attribute_name="organism">C</Attribute></Attributes>
</BioSample>
</BioSampleSet>"#;

        let (records, errors) = parse_all(xml);
        assert_eq!(records.len(), 2);
        assert_eq!(errors.len(), 1);
        assert_eq!(records[0].accession, "SAMN11111111");
        assert_eq!(records[1].accession, "SAMN33333333");
    }

    #[test]
    fn test_no_title() {
        let xml = r#"<BioSampleSet>
<BioSample accession="SAMN44444444" submission_date="2020-01-01">
  <Ids><Id db="BioSample">SAMN44444444</Id></Ids>
  <Description>
    <Comment><Paragraph>No title here</Paragraph></Comment>
  </Description>
  <Attributes>
    <Attribute attribute_name="organism">Test</Attribute>
  </Attributes>
</BioSample>
</BioSampleSet>"#;

        let (records, errors) = parse_all(xml);
        assert_eq!(errors.len(), 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].title, None);
    }

    #[test]
    fn test_html_entities_in_attribute_names() {
        let xml = r#"<BioSampleSet>
<BioSample accession="SAMN55555555" submission_date="2020-01-01">
  <Description><Title>Entities test</Title></Description>
  <Attributes>
    <Attribute attribute_name="&quot;PUBLIC&quot;">n</Attribute>
    <Attribute attribute_name="cDNA_adapter_5&apos;">none</Attribute>
  </Attributes>
</BioSample>
</BioSampleSet>"#;

        let (records, errors) = parse_all(xml);
        assert_eq!(errors.len(), 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].attributes[0].attribute_name, "\"PUBLIC\"");
        assert_eq!(records[0].attributes[1].attribute_name, "cDNA_adapter_5'");
    }

    #[test]
    fn test_multiple_records() {
        let xml = r#"<BioSampleSet>
<BioSample accession="SAMN00000001" submission_date="2020-01-01">
  <Description><Title>Record 1</Title></Description>
  <Attributes><Attribute attribute_name="organism">Alpha</Attribute></Attributes>
</BioSample>
<BioSample accession="SAMN00000002" submission_date="2020-01-02">
  <Description><Title>Record 2</Title></Description>
  <Attributes><Attribute attribute_name="organism">Beta</Attribute></Attributes>
</BioSample>
<BioSample accession="SAMN00000003" submission_date="2020-01-03">
  <Description><Title>Record 3</Title></Description>
  <Attributes><Attribute attribute_name="organism">Gamma</Attribute></Attributes>
</BioSample>
</BioSampleSet>"#;

        let (records, errors) = parse_all(xml);
        assert_eq!(errors.len(), 0);
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].accession, "SAMN00000001");
        assert_eq!(records[0].title.as_deref(), Some("Record 1"));
        assert_eq!(records[1].accession, "SAMN00000002");
        assert_eq!(records[2].accession, "SAMN00000003");
        assert_eq!(records[2].attributes[0].value.as_deref(), Some("Gamma"));
    }

    #[test]
    fn test_empty_attributes_section() {
        let xml = r#"<BioSampleSet>
<BioSample accession="SAMN66666666" submission_date="2020-01-01">
  <Description><Title>No attributes</Title></Description>
  <Attributes/>
</BioSample>
</BioSampleSet>"#;

        let (records, errors) = parse_all(xml);
        assert_eq!(errors.len(), 0);
        assert_eq!(records.len(), 1);
        assert!(records[0].attributes.is_empty());
    }

    #[test]
    fn test_bare_biosample_no_wrapper() {
        // Some fixtures like quotes.xml have no <BioSampleSet> wrapper
        let xml = r#"<BioSample accession="SAMEA0000001" submission_date="2020-01-01">
  <Description><Title>Bare record</Title></Description>
  <Attributes>
    <Attribute attribute_name="organism">Test</Attribute>
  </Attributes>
</BioSample>"#;

        let (records, errors) = parse_all(xml);
        assert_eq!(errors.len(), 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].accession, "SAMEA0000001");
    }
}
