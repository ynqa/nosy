use std::{ffi::OsStr, path::PathBuf};

use validator::ValidationError;

use crate::extractor::{self, pandoc::PANDOC_INSTALLATION_HINT};

/// Return error if extractor kind is invalid (e.g., required external command is missing)
pub fn validate_extractor_kind(kind: &extractor::Kind) -> Result<(), ValidationError> {
    match kind {
        extractor::Kind::HtmlNative => Ok(()),
        extractor::Kind::PdfNative => Ok(()),
        extractor::Kind::Pandoc => {
            validate_command_executable(OsStr::new("pandoc")).map_err(|_| {
                let mut err = ValidationError::new("missing_pandoc");
                err.message = Some(PANDOC_INSTALLATION_HINT.into());
                err
            })
        }
        extractor::Kind::Whisper => validate_whisper_model_path_from_env().map(|_| ()),
        _ => Ok(()),
    }
}

/// Return error if command is not executable or not found in PATH
pub fn validate_command_executable(command: &OsStr) -> Result<(), ValidationError> {
    let command = command.to_str().ok_or_else(|| {
        let mut err = ValidationError::new("invalid_utf8");
        err.message = Some("command contains invalid UTF-8".into());
        err
    })?;

    let command = command.trim();
    if command.is_empty() {
        let mut err = ValidationError::new("empty");
        err.message = Some("command is empty".into());
        return Err(err);
    }

    if which::which(command).is_err() {
        let mut err = ValidationError::new("command_not_found");
        err.message =
            Some(format!("`{command:?}` command is not executable or not in PATH").into());
        return Err(err);
    }

    Ok(())
}

/// Return error if file already exists at given path
pub fn validate_file_already_exists(path: &PathBuf) -> Result<(), ValidationError> {
    if path.exists() {
        let mut err = ValidationError::new("exists");
        err.message = Some(format!("file already exists at {path:?}").into());
        return Err(err);
    }
    Ok(())
}

/// Return error if file does not exist at given path
pub fn validate_file_not_exists(path: &PathBuf) -> Result<(), ValidationError> {
    if !path.exists() {
        let mut err = ValidationError::new("not_exists");
        err.message = Some(format!("file does not exist at {path:?}").into());
        return Err(err);
    }
    if !path.is_file() {
        let mut err = ValidationError::new("not_file");
        err.message = Some(format!("path is not a file: {path:?}").into());
        return Err(err);
    }
    Ok(())
}

/// Environment variable name for whisper model path
const WHISPER_MODEL_PATH_ENV: &str = "WHISPER_MODEL_PATH";

/// Return error if whisper model path does not exist or is not a file
/// Validate whisper settings and return the model path.
pub fn validate_whisper_model_path_from_env() -> Result<PathBuf, ValidationError> {
    let value = std::env::var(WHISPER_MODEL_PATH_ENV).map_err(|_| {
        let mut err = ValidationError::new("missing_env");
        err.message = Some(format!("{WHISPER_MODEL_PATH_ENV} is not set").into());
        err
    })?;
    if value.trim().is_empty() {
        let mut err = ValidationError::new("empty_env");
        err.message = Some(format!("{WHISPER_MODEL_PATH_ENV} is empty").into());
        return Err(err);
    }

    let path = PathBuf::from(value);
    match validate_file_not_exists(&path) {
        Ok(_) => Ok(path),
        Err(err) => {
            let mut new_err = ValidationError::new("invalid_whisper_model_path");
            new_err.message = Some(
                format!(
                    "invalid whisper model path at {path:?}: {}",
                    err.message.unwrap_or_default()
                )
                .into(),
            );
            Err(new_err)
        }
    }
}
