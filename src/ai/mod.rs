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
    pub fn new() -> Result<Self> {
        let system_prompt = load_system_prompt();

        // Prefer Anthropic if key is present, otherwise fall back to Ollama
        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            let model = std::env::var("HAMRS_MODEL")
                .unwrap_or_else(|_| DEFAULT_ANTHROPIC_MODEL.to_string());
            return Ok(Self {
                client: Client::new(),
                backend: Backend::Anthropic { api_key },
                model,
                system_prompt,
            });
        }

        let host = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| DEFAULT_OLLAMA_HOST.to_string());
        let model =
            std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| DEFAULT_OLLAMA_MODEL.to_string());

        eprintln!("  Using Ollama at {host} (model: {model})");
        eprintln!("  Set ANTHROPIC_API_KEY to use Claude instead.\n");

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

fn load_system_prompt() -> String {
    let config_path = dirs::config_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hamrs-ca")
        .join("system_prompt.md");

    std::fs::read_to_string(&config_path).unwrap_or_else(|_| DEFAULT_SYSTEM_PROMPT.to_string())
}
