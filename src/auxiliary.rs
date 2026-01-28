mod completion;
mod download_whisper;

pub use completion::CompletionArgs;
pub use download_whisper::DownloadWhisperArgs;

use crate::Command;

/// Handles auxiliary (e.g., completion) commands.
/// Returns Ok(true) if an auxiliary command was handled.
pub async fn handle_auxiliary_command(command: Option<&Command>) -> anyhow::Result<bool> {
    match command {
        Some(Command::Completion(args)) => {
            completion::handle(args)?;
            Ok(true)
        }
        Some(Command::DownloadWhisper(args)) => {
            download_whisper::handle(args).await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
