pub mod turtle;
pub mod ntriples;
pub mod jsonld;

use crate::model::BioSampleRecord;
use std::io::Write;

pub const PREFIX_SCHEMA: &str = "http://schema.org/";
pub const PREFIX_IDORG: &str = "http://identifiers.org/biosample/";
pub const PREFIX_DCT: &str = "http://purl.org/dc/terms/";
pub const PREFIX_DDBJ: &str = "http://ddbj.nig.ac.jp/biosample/";
pub const PREFIX_DDBJONT: &str = "http://ddbj.nig.ac.jp/ontologies/biosample/";
pub const PREFIX_RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
pub const PREFIX_XSD: &str = "https://www.w3.org/2001/XMLSchema#";

pub trait Serializer {
    fn write_header<W: Write>(&self, writer: &mut W) -> std::io::Result<()>;
    fn write_record<W: Write>(&self, writer: &mut W, record: &BioSampleRecord) -> std::io::Result<()>;
    fn write_footer<W: Write>(&self, writer: &mut W) -> std::io::Result<()>;
}
