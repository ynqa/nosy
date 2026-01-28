use anyhow::Context;
use clap::{Args, ValueEnum};
use genai::{
    adapter::AdapterKind,
    chat::{ChatMessage, ChatRequest},
    resolver::Endpoint,
};

/// LLM provider type; Copy from AdapterKind in genai crate
#[derive(Clone, Debug, ValueEnum)]
pub enum Provider {
    #[value(name = "github-copilot")]
    GitHubCopilot,

    /// For OpenAI Chat Completions and also can be used for OpenAI compatible APIs
    /// NOTE: This adapter share some behavior that other adapters can use while still providing some variant
    OpenAI,
    /// For OpenAI Responses API
    OpenAIResp,
    /// Gemini adapter supports gemini native protocol. e.g., support thinking budget.
    Gemini,
    /// Anthopric native protocol as well
    Anthropic,
    /// For fireworks.ai, mostly OpenAI.
    Fireworks,
    /// Together AI (Mostly uses OpenAI-compatible protocol)
    Together,
    /// Reuse some of the OpenAI adapter behavior, customize some (e.g., normalize thinking budget)
    Groq,
    /// For Mimo (Mostly use OpenAI)
    Mimo,
    /// For Nebius (Mostly use OpenAI)
    Nebius,
    /// For xAI (Mostly use OpenAI)
    Xai,
    /// For DeepSeek (Mostly use OpenAI)
    DeepSeek,
    /// For ZAI (Mostly use OpenAI)
    Zai,
    /// For big model (only accessible via namespace bigmodel::)
    BigModel,
    /// Cohere today use it's own native protocol but might move to OpenAI Adapter
    Cohere,
    /// OpenAI shared behavior + some custom. (currently, localhost only, can be customize with ServerTargetResolver).
    Ollama,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::GitHubCopilot => "github-copilot",
            Provider::OpenAI => "openai",
            Provider::OpenAIResp => "openai-resp",
            Provider::Gemini => "gemini",
            Provider::Anthropic => "anthropic",
            Provider::Fireworks => "fireworks",
            Provider::Together => "together",
            Provider::Groq => "groq",
            Provider::Mimo => "mimo",
            Provider::Nebius => "nebius",
            Provider::Xai => "xai",
            Provider::DeepSeek => "deepseek",
            Provider::Zai => "zai",
            Provider::BigModel => "bigmodel",
            Provider::Cohere => "cohere",
            Provider::Ollama => "ollama",
        }
    }
}

/// Options to construct LLM client
#[derive(Clone, Debug, Args)]
pub struct LLMConstructionOptions {
    #[arg(
        long = "provider",
        help = "LLM service provider",
        long_help = r#"LLM service provider.

API keys are resolved from environment variables according to genai conventions.
- If --provider is specified: use the default environment variable for that adapter
- If omitted: infer the adapter from the model name and use its default environment variable
  - If this fails, try specifying --provider explicitly.

Default environment variables (genai crate):
  - OpenAI / OpenAIResp: OPENAI_API_KEY
  - Anthropic: ANTHROPIC_API_KEY
  - Gemini: GEMINI_API_KEY
  - Fireworks: FIREWORKS_API_KEY
  - Together: TOGETHER_API_KEY
  - Groq: GROQ_API_KEY
  - Mimo: MIMO_API_KEY
  - Nebius: NEBIUS_API_KEY
  - xAI: XAI_API_KEY
  - DeepSeek: DEEPSEEK_API_KEY
  - ZAI: ZAI_API_KEY
  - BigModel: BIGMODEL_API_KEY
  - Cohere: COHERE_API_KEY
  - Ollama: (no API key required)

For the following AI providers, nosy uses a dedicated endpoint and reads a dedicated environment variable:
  - GitHub Copilot: GITHUB_COPILOT_API_KEY"#
    )]
    pub provider: Option<Provider>,
}

/// Create LLM client from LLMOptions
pub fn create_llm_client(opts: &LLMConstructionOptions) -> anyhow::Result<genai::Client> {
    let opts = opts.clone();
    Ok(genai::Client::builder()
        .with_service_target_resolver_fn(move |mut target: genai::ServiceTarget| {
            if let Some(Provider::GitHubCopilot) = &opts.provider {
                target.endpoint = Endpoint::from_static("https://models.inference.ai.azure.com");
                target.auth =
                    genai::resolver::AuthData::FromEnv("GITHUB_COPILOT_API_KEY".to_string());
            }
            Ok(target)
        })
        .build())
}

/// Options for LLM requests
#[derive(Clone, Debug, Args)]
pub struct LLMRequestOptions {
    #[arg(
        long = "model",
        default_value = "claude-sonnet-4-5-20250929",
        help = "LLM model identifier (e.g., claude-sonnet-4-5-20250929)"
    )]
    pub model: String,
}

/// Infer provider from model name via genai adapter mapping.
pub fn infer_adapter_kind(model: &str) -> anyhow::Result<AdapterKind> {
    AdapterKind::from_model(model)
        .with_context(|| format!("failed to infer provider from model '{model}'"))
}

/// Execute LLM chat request and return the first text response
pub async fn chat_request(
    client: &genai::Client,
    opts: &LLMRequestOptions,
    messages: Vec<ChatMessage>,
) -> anyhow::Result<String> {
    let chat_resp = client
        .exec_chat(&opts.model, ChatRequest::new(messages), None)
        .await
        .with_context(|| format!("failed to execute chat request (model: {})", opts.model))?;

    chat_resp
        .first_text()
        .map(|s| s.to_string())
        .context("LLM returned no text")
}
