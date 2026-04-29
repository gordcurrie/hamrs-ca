use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const DEFAULT_ANTHROPIC_MODEL: &str = "claude-sonnet-4-6";
const DEFAULT_OLLAMA_MODEL: &str = "glm-4.7-flash";
const DEFAULT_OLLAMA_HOST: &str = "http://localhost:11434";

const DEFAULT_SYSTEM_PROMPT: &str = "\
You are a technical instructor for the Canadian Amateur Radio Basic Qualification exam. \
Explain concepts at an engineering level — precise, accurate, and substantive. \
Do not dumb things down, but do not assume prior RF knowledge either.

For each concept:
1. Explain the underlying physics or engineering principle clearly and precisely.
2. Show how the principle connects to real amateur radio practice — why it matters on the air.
3. State the exam-critical facts explicitly — what a candidate must know to answer correctly.
4. Address common misconceptions if they exist.

Be dense and precise. Aim for the depth of a good RF engineering textbook. No padding.

Use markdown formatting. For bullet points always use `- ` (dash space), never `*   ` with indentation. \
Keep bullet content on a single line without indentation.";

#[derive(Deserialize, Default)]
struct HamrsConfig {
    anthropic_api_key: Option<String>,
    model: Option<String>,
    ollama_host: Option<String>,
    ollama_model: Option<String>,
}

fn load_config() -> HamrsConfig {
    let path = dirs::config_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hamrs-ca")
        .join("config.toml");

    load_config_from(&path)
}

fn load_config_from(path: &std::path::Path) -> HamrsConfig {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

enum Backend {
    Anthropic { api_key: String },
    Ollama { host: String },
}

pub struct ConceptClient {
    client: Client,
    backend: Backend,
    model: String,
    system_prompt: String,
}

// Anthropic wire types
#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicBlock>,
}

#[derive(Deserialize)]
struct AnthropicBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

// Ollama wire types
#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    message: OllamaMessage,
}

#[derive(Deserialize)]
struct OllamaMessage {
    content: String,
}

// Shared message type — role is always one of "system" / "user" / "assistant"
#[derive(Serialize, Clone)]
pub struct Message {
    pub role: &'static str,
    pub content: String,
}

impl ConceptClient {
    pub async fn is_available() -> bool {
        let config = load_config();

        let has_anthropic =
            config.anthropic_api_key.is_some() || std::env::var("HAMRS_ANTHROPIC_API_KEY").is_ok();
        if has_anthropic {
            return true;
        }

        let host = config
            .ollama_host
            .or_else(|| std::env::var("OLLAMA_HOST").ok())
            .unwrap_or_else(|| DEFAULT_OLLAMA_HOST.to_string());

        let Ok(client) = Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
        else {
            return false;
        };
        let available = client
            .get(format!("{host}/api/tags"))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);

        if !available {
            ensure_config_exists();
            let config_path = dirs::config_local_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("hamrs-ca")
                .join("config.toml");
            eprintln!();
            eprintln!("  No AI backend found — follow-up questions are disabled.");
            eprintln!("  To enable them, edit: {}", config_path.display());
            eprintln!();
        }

        available
    }

    pub fn new() -> Result<Self> {
        let config = load_config();
        let system_prompt = load_system_prompt();

        // Prefer Anthropic if a key is configured, otherwise fall back to Ollama
        let anthropic_key = config
            .anthropic_api_key
            .or_else(|| std::env::var("HAMRS_ANTHROPIC_API_KEY").ok());

        if let Some(api_key) = anthropic_key {
            let model = config
                .model
                .or_else(|| std::env::var("HAMRS_MODEL").ok())
                .unwrap_or_else(|| DEFAULT_ANTHROPIC_MODEL.to_string());
            return Ok(Self {
                client: Client::new(),
                backend: Backend::Anthropic { api_key },
                model,
                system_prompt,
            });
        }

        let host = config
            .ollama_host
            .or_else(|| std::env::var("OLLAMA_HOST").ok())
            .unwrap_or_else(|| DEFAULT_OLLAMA_HOST.to_string());
        let model = config
            .ollama_model
            .or_else(|| std::env::var("OLLAMA_MODEL").ok())
            .unwrap_or_else(|| DEFAULT_OLLAMA_MODEL.to_string());

        eprintln!("  Using Ollama at {host} (model: {model})");
        eprintln!(
            "  Add anthropic_api_key to ~/.config/hamrs-ca/config.toml to use Claude instead.\n"
        );

        Ok(Self {
            client: Client::new(),
            backend: Backend::Ollama { host },
            model,
            system_prompt,
        })
    }

    pub async fn explain(&self, messages: Vec<Message>) -> Result<String> {
        match &self.backend {
            Backend::Anthropic { api_key } => self.explain_anthropic(api_key, messages).await,
            Backend::Ollama { host } => self.explain_ollama(host, messages).await,
        }
    }

    async fn explain_anthropic(&self, api_key: &str, messages: Vec<Message>) -> Result<String> {
        let body = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: 2048,
            system: self.system_prompt.clone(),
            messages,
        };

        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("API error {status}: {text}"));
        }

        let parsed: AnthropicResponse = resp.json().await?;
        parsed
            .content
            .into_iter()
            .find(|b| b.kind == "text")
            .map(|b| b.text)
            .ok_or_else(|| anyhow!("empty response from Anthropic"))
    }

    async fn explain_ollama(&self, host: &str, messages: Vec<Message>) -> Result<String> {
        // Ollama puts the system prompt as the first message rather than a top-level field
        let mut all_messages = vec![Message {
            role: "system",
            content: self.system_prompt.clone(),
        }];
        all_messages.extend(messages);

        let body = OllamaRequest {
            model: self.model.clone(),
            messages: all_messages,
            stream: false,
        };

        let url = format!("{host}/api/chat");
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                anyhow!(
                    "Could not reach Ollama at {host}: {e}\nIs Ollama running? Try: ollama serve"
                )
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Ollama error {status}: {text}"));
        }

        let parsed: OllamaResponse = resp.json().await?;
        Ok(parsed.message.content)
    }
}

const EXAMPLE_CONFIG: &str = r#"# hamrs-ca configuration
#
# Learn mode supports two AI backends for follow-up questions.
# Uncomment and fill in one of the options below.

# --- Option A: Anthropic (Claude) ---
# Get a key at https://console.anthropic.com
#
# anthropic_api_key = "sk-ant-..."
# model = "claude-sonnet-4-6"          # optional, this is the default

# --- Option B: Ollama (local, no API key needed) ---
# Install Ollama from https://ollama.com, then: ollama pull glm-4.7-flash
#
# ollama_host = "http://localhost:11434"   # optional, this is the default
# ollama_model = "glm-4.7-flash"          # optional, this is the default
"#;

fn ensure_config_exists() {
    let Some(config_dir) = dirs::config_local_dir() else {
        return;
    };
    ensure_config_at(&config_dir.join("hamrs-ca").join("config.toml"));
}

fn ensure_config_at(path: &std::path::Path) {
    if path.exists() {
        return;
    }
    if let Some(dir) = path.parent() {
        if std::fs::create_dir_all(dir).is_ok() {
            let _ = std::fs::write(path, EXAMPLE_CONFIG);
        }
    }
}

fn load_system_prompt() -> String {
    let config_path = dirs::config_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hamrs-ca")
        .join("system_prompt.md");

    std::fs::read_to_string(&config_path).unwrap_or_else(|_| DEFAULT_SYSTEM_PROMPT.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_to_none_when_file_missing() {
        let config = load_config_from(std::path::Path::new("/nonexistent/config.toml"));
        assert!(config.anthropic_api_key.is_none());
        assert!(config.model.is_none());
        assert!(config.ollama_host.is_none());
        assert!(config.ollama_model.is_none());
    }

    #[test]
    fn config_parses_anthropic_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "anthropic_api_key = \"sk-ant-test\"\n").unwrap();
        let config = load_config_from(&path);
        assert_eq!(config.anthropic_api_key.as_deref(), Some("sk-ant-test"));
        assert!(config.model.is_none());
    }

    #[test]
    fn config_parses_all_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "anthropic_api_key = \"sk-ant-x\"\nmodel = \"claude-opus-4-7\"\nollama_host = \"http://10.0.0.1:11434\"\nollama_model = \"gemma4\"\n",
        )
        .unwrap();
        let config = load_config_from(&path);
        assert_eq!(config.anthropic_api_key.as_deref(), Some("sk-ant-x"));
        assert_eq!(config.model.as_deref(), Some("claude-opus-4-7"));
        assert_eq!(config.ollama_host.as_deref(), Some("http://10.0.0.1:11434"));
        assert_eq!(config.ollama_model.as_deref(), Some("gemma4"));
    }

    #[test]
    fn config_ignores_unknown_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "unknown_key = \"value\"\n").unwrap();
        let config = load_config_from(&path);
        assert!(config.anthropic_api_key.is_none());
    }

    #[test]
    fn ensure_config_creates_file_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("subdir").join("config.toml");
        assert!(!path.exists());
        ensure_config_at(&path);
        assert!(path.exists());
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("anthropic_api_key"));
        assert!(contents.contains("ollama_host"));
    }

    #[test]
    fn ensure_config_does_not_overwrite_existing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "anthropic_api_key = \"sk-ant-keep\"\n").unwrap();
        ensure_config_at(&path);
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "anthropic_api_key = \"sk-ant-keep\"\n");
    }
}
