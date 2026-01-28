use std::path::PathBuf;

use anyhow::Context;
use clap::Args;
use genai::chat::{ChatMessage, ChatRole};
use handlebars::Handlebars;
use validator::Validate;

use crate::validate::validate_file_not_exists;

pub const DEFAULT_SYSTEM_TEMPLATE: &str = include_str!("../assets/system.hbs");
pub const DEFAULT_USER_TEMPLATE: &str = include_str!("../assets/user.hbs");

/// Options to create chat messages
#[derive(Debug, Clone, Args, Validate)]
pub struct ChatMessageOptions {
    #[arg(
        long = "system-template",
        value_name = "PATH",
        help = "Path to the system message template file (defaults to built-in template)"
    )]
    #[validate(custom(function = "validate_file_not_exists"))]
    pub system_template: Option<PathBuf>,

    #[arg(
        long = "user-template",
        value_name = "PATH",
        help = "Path to the user message template file (defaults to built-in template)"
    )]
    #[validate(custom(function = "validate_file_not_exists"))]
    pub user_template: Option<PathBuf>,
}

/// Variables for chat message templates with system role
#[derive(Debug, Clone, Args, Validate, serde::Serialize)]
pub struct SystemChatMessageVariables {
    #[arg(
        long = "lang",
        default_value = "English",
        help = "Language for the summary"
    )]
    #[serde(rename = "language")]
    pub language: String,
}

/// Create system and user chat messages from templates and variables.
pub fn create_chat_messages(
    opts: &ChatMessageOptions,
    system_vars: &impl serde::Serialize,
    user_vars: &impl serde::Serialize,
) -> anyhow::Result<Vec<ChatMessage>> {
    opts.validate()
        .map_err(|err| anyhow::anyhow!(err.to_string()))?;
    let system_template = match &opts.system_template {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("failed to read system template file: {path:?}"))?,
        None => DEFAULT_SYSTEM_TEMPLATE.to_string(),
    };
    let user_template = match &opts.user_template {
        Some(path) => std::fs::read_to_string(path)
            .with_context(|| format!("failed to read user template file: {path:?}"))?,
        None => DEFAULT_USER_TEMPLATE.to_string(),
    };

    let system_message = create_message(ChatRole::System, &system_template, system_vars)?;
    let user_message = create_message(ChatRole::User, &user_template, user_vars)?;

    Ok(vec![system_message, user_message])
}

/// Create a chat message from a template and variables.
fn create_message(
    role: ChatRole,
    template: &str,
    variables: &impl serde::Serialize,
) -> anyhow::Result<ChatMessage> {
    let mut handlebars = Handlebars::new();
    handlebars.register_template_string("template", template)?;
    let content = handlebars.render("template", variables)?;

    Ok(ChatMessage {
        role,
        content: content.into(),
        options: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    mod create_chat_messages {
        use super::*;

        #[test]
        fn defaults_to_built_in_templates() {
            #[derive(serde::Serialize)]
            struct SystemVars {
                language: String,
            }

            #[derive(serde::Serialize)]
            struct UserVars {
                content: String,
            }

            let system_vars = SystemVars {
                language: "English".to_string(),
            };
            let user_vars = UserVars {
                content: "Hello world".to_string(),
            };

            let opts = ChatMessageOptions {
                system_template: None,
                user_template: None,
            };

            let messages = create_chat_messages(&opts, &system_vars, &user_vars).unwrap();
            assert_eq!(messages.len(), 2);
            assert!(matches!(messages[0].role, ChatRole::System));
            assert!(matches!(messages[1].role, ChatRole::User));
        }
    }

    mod create_message {
        use super::*;

        #[test]
        fn test_create_message() {
            #[derive(serde::Serialize)]
            struct Vars {
                name: String,
            }

            let vars = Vars {
                name: "Alice".to_string(),
            };

            let message = create_message(ChatRole::User, "Hello, {{name}}!", &vars).unwrap();

            assert!(matches!(message.role, ChatRole::User));
            assert_eq!(message.content.first_text(), Some("Hello, Alice!"));
        }
    }
}
