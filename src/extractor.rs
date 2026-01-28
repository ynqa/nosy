use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use crate::file_type;
use clap::ValueEnum;
use indicatif::ProgressBar;

pub mod html;
pub mod pandoc;
pub mod pdf;
pub mod whisper;

pub const EXTRACTED_CONTENT_FILENAME: &str = "ext";

/// A trait for extracting text content from various formats.
#[async_trait::async_trait]
pub trait Extractor {
    /// Extract text content from the given byte slice.
    async fn extract(
        &self,
        content_path: &Path,
        extension: &Option<file_type::Extension>,
        mime: &Option<file_type::Mime>,
        workdir: &Path,
        bar: &ProgressBar,
    ) -> anyhow::Result<PathBuf>;
}

/// Kind of extractor
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum, serde::Serialize)]
pub enum Kind {
    /// Pass-through for plain text inputs
    #[value(name = "plain")]
    PlainText,
    /// Use built-in (readability-like) HTML extractor
    #[value(name = "html")]
    HtmlNative,
    /// Use built-in PDF text extractor
    #[value(name = "pdf")]
    PdfNative,
    /// Use pandoc command line tool for supported document formats
    #[value(name = "pandoc")]
    Pandoc,
    /// Use whisper for audio/video transcription
    /// WHISPER_MODEL_PATH environment variable must be set to a valid whisper model file path
    #[value(name = "whisper")]
    Whisper,
    #[value(skip)]
    Unsupported,
}

macro_rules! define_indices {
    (
        $(
            $variant:ident => {
                mime: [$( $mime:expr ),* $(,)?],
                ext: [$( $ext:expr ),* $(,)?] $(,)?
            }
        ),* $(,)?
    ) => {
        pub static MIME_INDEX: LazyLock<HashMap<file_type::Mime, Kind>> = LazyLock::new(|| {
            let mut map = HashMap::new();
            $(
                $( map.insert(file_type::Mime($mime.to_string()), Kind::$variant); )*
            )*
            map
        });

        pub static EXT_INDEX: LazyLock<HashMap<file_type::Extension, Kind>> = LazyLock::new(|| {
            let mut map = HashMap::new();
            $(
                $( map.insert(file_type::Extension($ext.to_string()), Kind::$variant); )*
            )*
            map
        });
    };
}

// Mapping indices for MIME types and file extensions to extractor kinds
// Aim to map MIME types / file extensions commonly used for text extraction
//
// References:
// - https://mimetype.io/all-types
define_indices! {
    HtmlNative => {
        mime: ["text/html", "application/xhtml+xml"],
        ext: ["html", "htm", "xhtml"],
    },
    PdfNative => {
        mime: ["application/pdf"],
        ext: ["pdf"],
    },
    PlainText => {
        mime: ["text/plain", "text/markdown"],
        ext: ["txt", "text", "md"],
    },
    Pandoc => {
        mime: [
            // docx
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            // doc
            "application/msword",
            // odt
            "application/vnd.oasis.opendocument.text",
            // rtf
            "application/rtf",
            "text/rtf",
            // epub
            "application/epub+zip",
            // latex
            "text/latex",
            // tex
            "application/x-tex",
            "text/x-tex",
        ],
        ext: ["docx", "doc", "odt", "rtf", "epub", "tex", "latex"],
    },
    Whisper => {
        mime: [
            // mp3
            "audio/mpeg",
            "audio/mp3",
            "audio/x-mp3",
            // wav
            "audio/wav",
            "audio/x-wav",
            // mp4 (audio/video)
            "audio/mp4",
            "video/mp4",
        ],
        ext: ["mp3", "wav", "mp4", "m4a"],
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    mod define_indices {
        use super::*;

        #[test]
        fn test() {
            define_indices! {
                HtmlNative => {
                    mime: ["text/html", "application/xhtml+xml"],
                    ext: ["html", "htm"],
                },
                PdfNative => {
                    mime: ["application/pdf"],
                    ext: ["pdf"],
                },
            }

            assert_eq!(
                MIME_INDEX.get(&file_type::Mime("text/html".to_string())),
                Some(&Kind::HtmlNative)
            );
            assert_eq!(
                MIME_INDEX.get(&file_type::Mime("application/pdf".to_string())),
                Some(&Kind::PdfNative)
            );
            assert_eq!(
                MIME_INDEX.get(&file_type::Mime("application/unknown".to_string())),
                None
            );

            assert_eq!(
                EXT_INDEX.get(&file_type::Extension("html".to_string())),
                Some(&Kind::HtmlNative)
            );
            assert_eq!(
                EXT_INDEX.get(&file_type::Extension("pdf".to_string())),
                Some(&Kind::PdfNative)
            );
            assert_eq!(
                EXT_INDEX.get(&file_type::Extension("unknown".to_string())),
                None
            );
        }
    }
}
