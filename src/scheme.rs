/// Scheme of input
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputScheme {
    Http,
    File,
    Unsupported,
}

/// Detect input scheme from input string
pub fn detect(input: &str) -> InputScheme {
    if let Some((scheme, _)) = input.split_once("://") {
        match scheme.to_ascii_lowercase().as_str() {
            "http" | "https" => InputScheme::Http,
            "file" => InputScheme::File,
            _ => InputScheme::Unsupported,
        }
    } else {
        InputScheme::File
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod scheme_detector {
        use super::*;

        mod detect {
            use super::*;

            #[test]
            fn http() {
                let input = "http://example.com";
                let scheme = detect(input);
                assert_eq!(scheme, InputScheme::Http);
            }

            #[test]
            fn https() {
                let input = "https://example.com";
                let scheme = detect(input);
                assert_eq!(scheme, InputScheme::Http);
            }

            #[test]
            fn file() {
                let input = "file:///path/to/file.txt";
                let scheme = detect(input);
                assert_eq!(scheme, InputScheme::File);
            }

            #[test]
            fn unknown() {
                let input = "fake://example.com";
                let scheme = detect(input);
                assert_eq!(scheme, InputScheme::Unsupported);
            }

            #[test]
            fn no_scheme() {
                let input = "/path/to/file.txt";
                let scheme = detect(input);
                assert_eq!(scheme, InputScheme::File);
            }
        }
    }
}
