use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::Context;

use crate::extractor::{self, EXT_INDEX, MIME_INDEX};

/// Representation of MIME type
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Mime(pub String);

impl Mime {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for Mime {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Representation of file extension
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Extension(pub String);

impl Extension {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for Extension {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Number of bytes to read for MIME sniffing
const MIME_SNIFF_BYTES: usize = 8 * 1024;

/// Get MIME type of a file by sniffing
pub fn mime_type(path: &PathBuf) -> anyhow::Result<Mime> {
    match read_prefix(path) {
        Ok(content) => {
            let mime = tree_magic_mini::from_u8(&content);
            Ok(mime.to_string().into())
        }
        Err(err) => Err(err).context(format!(
            "failed to read file prefix for MIME sniffing: '{path:?}'"
        )),
    }
}

/// Match kind of extractor by MIME type
pub fn match_kind_by_mime(mime: &Option<Mime>) -> extractor::Kind {
    mime.as_ref()
        .and_then(|m| MIME_INDEX.get(m).copied())
        .unwrap_or(extractor::Kind::Unsupported)
}

/// Get lowercase file extension from path
pub fn file_extension_lowercase(path: &Path) -> Option<Extension> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .map(Into::into)
}

/// Match kind of extractor by file extension
pub fn match_kind_by_extension(extension: &Option<Extension>) -> extractor::Kind {
    extension
        .as_ref()
        .and_then(|ext| EXT_INDEX.get(ext).copied())
        .unwrap_or(extractor::Kind::Unsupported)
}

/// Read the prefix bytes of a file for MIME sniffing
fn read_prefix(path: &PathBuf) -> std::io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let mut buf = Vec::with_capacity(MIME_SNIFF_BYTES);
    file.take(MIME_SNIFF_BYTES as u64).read_to_end(&mut buf)?;
    Ok(buf)
}
