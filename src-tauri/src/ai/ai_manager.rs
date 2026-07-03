use serde::{Deserialize, Serialize};
use reqwest::Client;
use crate::models::AppSettings;
use tokio::sync::Mutex;
use std::sync::Arc;
use std::path::PathBuf;
use crate::ai::builtin_inference::BuiltinInference;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

pub struct AiManager {
    client: Client,
    builtin: Arc<Mutex<Option<BuiltinInference>>>,
    cancel_flag: Arc<AtomicBool>,
    current_model_path: Arc<Mutex<Option<String>>>,
}

// ── Ollama chat API ──────────────────────────────────────────────────────────

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct OllamaChatResponse {
    message: ChatMessage,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
    #[serde(default)]
    eval_count: Option<u32>,
}

/// Token usage for a single generation call.
#[derive(Default, Clone, Copy)]
struct AiUsage {
    tokens_in: u32,
    tokens_out: u32,
}

#[derive(Serialize, Deserialize, Clone)]
struct ChatMessage {
    role: String,
    content: String,
}

// ── OpenAI chat completions API ──────────────────────────────────────────────

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize, Default)]
struct OpenAiUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: ChatMessage,
}

// ── Anthropic Messages API ───────────────────────────────────────────────────

#[derive(Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ChatMessage>,
}

#[derive(Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
    #[serde(default)]
    usage: Option<ClaudeUsage>,
}

#[derive(Deserialize, Default)]
struct ClaudeUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

#[derive(Deserialize)]
struct ClaudeContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
}

// ────────────────────────────────────────────────────────────────────────────

impl AiManager {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            builtin: Arc::new(Mutex::new(None)),
            current_model_path: Arc::new(Mutex::new(None)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn generate_sql(
        &self,
        app_handle: &tauri::AppHandle,
        settings: &AppSettings,
        schema_json: &str,
        dialect: &str,
        human_input: &str,
        model_name: Option<String>,
        exclude_patterns: &[String],
    ) -> Result<crate::models::AiSqlResult, String> {
        self.cancel_flag.store(false, Ordering::SeqCst);
        let ai_mode = settings.ai_mode.as_deref().unwrap_or("integrated");

        // Single-call strategy: send the FULL schema (all tables/views) in one request.
        // The only reduction applied is the user-configured exclude list (e.g. backup /
        // pre-delete tables), which trims pure noise without a separate selection step.
        let objects: Vec<crate::models::DbObject> =
            serde_json::from_str(schema_json).unwrap_or_default();
        let tables: Vec<crate::models::DbObject> = objects
            .into_iter()
            .filter(|o| o.object_type == "table" || o.object_type == "view")
            .filter(|o| !Self::is_excluded(&o.name, exclude_patterns))
            .collect();
        let table_count = tables.len() as u32;
        let schema_str = self.compact_schema(tables);

        let (system_prompt, user_message) = self.build_prompt_parts(dialect, &schema_str, human_input);

        // Resolve a human-readable model label before `model_name` is moved into the call.
        let model_label = match ai_mode {
            "builtin" => model_name.clone().unwrap_or_else(|| "model.gguf".to_string()),
            "integrated" => settings
                .ollama_model
                .clone()
                .unwrap_or_else(|| "qwen2.5:7b".to_string()),
            "cloud" => settings.cloud_model.clone().unwrap_or_default(),
            _ => String::new(),
        };

        let start = std::time::Instant::now();
        let (raw, usage) = match ai_mode {
            "builtin" => {
                // Qwen2.5 chat template
                let prompt = format!(
                    "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
                    system_prompt, user_message
                );
                self.call_builtin(app_handle, &prompt, model_name).await?
            }
            "integrated" => self.call_ollama(settings, &system_prompt, &user_message).await?,
            "cloud" => self.call_cloud(settings, &system_prompt, &user_message).await?,
            _ => return Err(format!("AI mode '{}' not supported", ai_mode)),
        };
        let time_ms = start.elapsed().as_millis() as u64;

        Ok(crate::models::AiSqlResult {
            sql: self.clean_sql(&raw),
            tokens_in: usage.tokens_in,
            tokens_out: usage.tokens_out,
            time_ms,
            model: model_label,
            table_count,
        })
    }

    /// Build (system_prompt, user_message) for chat-style APIs.
    fn build_prompt_parts(
        &self,
        dialect: &str,
        schema_str: &str,
        human_input: &str,
    ) -> (String, String) {
        let hints = Self::dialect_hints(dialect);
        let hint_section = if hints.is_empty() {
            String::new()
        } else {
            format!("\n\nDialect rules:\n{}", hints)
        };

        let system = format!(
            "You are a {} SQL expert. Convert the user's natural language question into a correct, \
             executable {} SQL query.\
             \nOutput ONLY the SQL query — no explanation, no markdown fences, no comments.{}",
            dialect, dialect, hint_section
        );

        let user = format!(
            "Database schema:\n{}\nQuestion: {}",
            schema_str, human_input
        );

        (system, user)
    }

    /// Roughly estimate token count (~4 chars/token) when a provider omits usage data.
    fn estimate_tokens(text: &str) -> u32 {
        ((text.chars().count() as f32) / 4.0).ceil() as u32
    }

    /// Returns true if `name` matches any exclude pattern. Patterns support a single
    /// leading or trailing `*` wildcard; otherwise a case-insensitive substring match.
    fn is_excluded(name: &str, patterns: &[String]) -> bool {
        if patterns.is_empty() {
            return false;
        }
        let lname = name.to_lowercase();
        patterns.iter().any(|p| {
            let p = p.trim().to_lowercase();
            if p.is_empty() {
                false
            } else if let Some(prefix) = p.strip_suffix('*') {
                lname.starts_with(prefix)
            } else if let Some(suffix) = p.strip_prefix('*') {
                lname.ends_with(suffix)
            } else {
                lname.contains(&p)
            }
        })
    }

    fn dialect_hints(dialect: &str) -> &'static str {
        match dialect {
            "MySQL" | "MariaDB" => {
                "- Use LIMIT N for row limits (not TOP)\n\
                 - Use backtick quotes for reserved word identifiers\n\
                 - Use NOW() for current timestamp"
            }
            "PostgreSQL" => {
                "- Use LIMIT N OFFSET M for pagination\n\
                 - Use double-quote identifiers when needed\n\
                 - Use NOW() or CURRENT_TIMESTAMP"
            }
            "SQL Server" | "SqlServer" | "MSSQL" => {
                "- Use TOP N or OFFSET M ROWS FETCH NEXT N ROWS ONLY\n\
                 - Use square-bracket identifiers [col] for reserved words\n\
                 - Use GETDATE() for current timestamp"
            }
            "SQLite" => {
                "- Use LIMIT N OFFSET M\n\
                 - No BOOLEAN type; use INTEGER 0/1\n\
                 - Use datetime('now') for current timestamp"
            }
            "Oracle" => {
                "- Use FETCH FIRST N ROWS ONLY or ROWNUM <= N\n\
                 - Use single quotes only; no double-quote string literals\n\
                 - Use SYSDATE for current date"
            }
            _ => "",
        }
    }

    /// Compact schema with PK/FK heuristic markers.
    pub fn compact_schema(&self, objects: Vec<crate::models::DbObject>) -> String {
        let mut out = String::new();
        for obj in &objects {
            let table_lower = obj.name.to_lowercase();
            let cols: Vec<String> = obj
                .columns
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|c| {
                    let col_lower = c.name.to_lowercase();
                    // Heuristic: "id" or "<table>_id" → PK; ends with "_id" → FK
                    let is_pk = col_lower == "id"
                        || col_lower == format!("{}_id", table_lower);
                    let is_fk = !is_pk && col_lower.ends_with("_id");
                    let marker = if is_pk {
                        " [PK]"
                    } else if is_fk {
                        " [FK]"
                    } else {
                        ""
                    };
                    format!("{}:{}{}", c.name, c.data_type, marker)
                })
                .collect();

            let desc = obj
                .description
                .as_deref()
                .filter(|d| !d.is_empty())
                .map(|d| format!(" -- {}", d.replace('\n', " ")))
                .unwrap_or_default();

            out.push_str(&format!("{}: ({}){}\n", obj.name, cols.join(", "), desc));
        }
        out
    }

    pub fn get_models_directory(&self) -> Result<PathBuf, String> {
        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        let models_dir = home.join(".vidbconnect").join("models");
        if !models_dir.exists() {
            std::fs::create_dir_all(&models_dir).map_err(|e| e.to_string())?;
        }
        Ok(models_dir)
    }

    pub fn list_local_models(&self) -> Result<Vec<String>, String> {
        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        let models_dir = home.join(".vidbconnect").join("models");

        if !models_dir.exists() {
            return Ok(Vec::new());
        }

        let mut models = Vec::new();
        if let Ok(entries) = std::fs::read_dir(models_dir) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_file() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.ends_with(".gguf") {
                            models.push(name);
                        }
                    }
                }
            }
        }
        Ok(models)
    }

    pub fn cancel_generation(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    // ── Provider implementations ─────────────────────────────────────────────

    async fn call_builtin(
        &self,
        app_handle: &tauri::AppHandle,
        prompt: &str,
        model_name: Option<String>,
    ) -> Result<(String, AiUsage), String> {
        use tauri::Emitter;

        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        let models_dir = home.join(".vidbconnect").join("models");

        let target = model_name.unwrap_or_else(|| "model.gguf".to_string());
        let model_path = models_dir.join(&target);
        let model_path_str = model_path.to_string_lossy().to_string();

        if !model_path.exists() {
            return Err(format!("Model file not found: {}", model_path.display()));
        }

        let mut builtin_lock = self.builtin.lock().await;
        let mut path_lock = self.current_model_path.lock().await;

        if builtin_lock.is_none() || path_lock.as_ref() != Some(&model_path_str) {
            let _ = app_handle.emit(
                "ai-progress",
                format!("Loading local model {} (this may take a moment)...", target),
            );
            let inference = BuiltinInference::new(model_path)?;
            *builtin_lock = Some(inference);
            *path_lock = Some(model_path_str);
            let _ = app_handle.emit("ai-progress", "Model loaded.");
        }

        let inference = builtin_lock.as_mut().unwrap();
        let (text, prompt_tokens, completion_tokens) = inference
            .generate(prompt, 512, app_handle, self.cancel_flag.clone())
            .await?;
        Ok((
            text,
            AiUsage {
                tokens_in: prompt_tokens as u32,
                tokens_out: completion_tokens as u32,
            },
        ))
    }

    /// Ollama — uses /api/chat with system + user messages.
    async fn call_ollama(
        &self,
        settings: &AppSettings,
        system: &str,
        user: &str,
    ) -> Result<(String, AiUsage), String> {
        let base_url = settings.ollama_url.as_deref().unwrap_or("http://localhost:11434");
        let url = format!("{}/api/chat", base_url.trim_end_matches('/'));
        let model = settings.ollama_model.as_deref().unwrap_or("qwen2.5:7b");

        let response = self
            .client
            .post(url)
            .json(&OllamaChatRequest {
                model: model.to_string(),
                messages: vec![
                    ChatMessage { role: "system".into(), content: system.to_string() },
                    ChatMessage { role: "user".into(), content: user.to_string() },
                ],
                stream: false,
            })
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let err = response.text().await.unwrap_or_else(|_| "Unknown error".into());
            return Err(format!("Ollama error: {}", err));
        }

        let res: OllamaChatResponse = response.json().await.map_err(|e| e.to_string())?;
        let usage = AiUsage {
            tokens_in: res
                .prompt_eval_count
                .unwrap_or_else(|| Self::estimate_tokens(system) + Self::estimate_tokens(user)),
            tokens_out: res
                .eval_count
                .unwrap_or_else(|| Self::estimate_tokens(&res.message.content)),
        };
        Ok((res.message.content, usage))
    }

    async fn call_cloud(
        &self,
        settings: &AppSettings,
        system: &str,
        user: &str,
    ) -> Result<(String, AiUsage), String> {
        let provider = settings.cloud_provider.as_deref().unwrap_or("gemini");
        match provider {
            "gemini" => self.call_gemini(settings, system, user).await,
            "openai" => self.call_openai(settings, system, user).await,
            "claude" => self.call_claude(settings, system, user).await,
            _ => Err(format!("Cloud provider '{}' is not supported", provider)),
        }
    }

    async fn call_gemini(
        &self,
        settings: &AppSettings,
        system: &str,
        user: &str,
    ) -> Result<(String, AiUsage), String> {
        let api_key = settings.cloud_api_key.as_deref().ok_or("Gemini API key missing")?;
        let model = settings.cloud_model.as_deref().unwrap_or("gemini-1.5-flash");
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, api_key
        );

        let body = serde_json::json!({
            "system_instruction": {
                "parts": [{ "text": system }]
            },
            "contents": [{
                "role": "user",
                "parts": [{ "text": user }]
            }]
        });

        let response = self
            .client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let err = response.text().await.unwrap_or_else(|_| "Unknown error".into());
            return Err(format!("Gemini API error: {}", err));
        }

        let res: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
        let text = res["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or("Failed to extract text from Gemini response")?
            .to_string();

        let usage = AiUsage {
            tokens_in: res["usageMetadata"]["promptTokenCount"]
                .as_u64()
                .map(|n| n as u32)
                .unwrap_or_else(|| Self::estimate_tokens(system) + Self::estimate_tokens(user)),
            tokens_out: res["usageMetadata"]["candidatesTokenCount"]
                .as_u64()
                .map(|n| n as u32)
                .unwrap_or_else(|| Self::estimate_tokens(&text)),
        };
        Ok((text, usage))
    }

    async fn call_openai(
        &self,
        settings: &AppSettings,
        system: &str,
        user: &str,
    ) -> Result<(String, AiUsage), String> {
        let api_key = settings.cloud_api_key.as_deref().ok_or("OpenAI API key missing")?;
        let model = settings.cloud_model.as_deref().unwrap_or("gpt-4o-mini");
        let base_url = settings
            .cloud_base_url
            .as_deref()
            .unwrap_or("https://api.openai.com");
        let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&OpenAiRequest {
                model: model.to_string(),
                messages: vec![
                    ChatMessage { role: "system".into(), content: system.to_string() },
                    ChatMessage { role: "user".into(), content: user.to_string() },
                ],
                temperature: 0.0,
            })
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let err = response.text().await.unwrap_or_else(|_| "Unknown error".into());
            return Err(format!("OpenAI API error: {}", err));
        }

        let res: OpenAiResponse = response.json().await.map_err(|e| e.to_string())?;
        let usage_data = res.usage.unwrap_or_default();
        let text = res
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or("Empty response from OpenAI")?;

        let usage = AiUsage {
            tokens_in: if usage_data.prompt_tokens > 0 {
                usage_data.prompt_tokens
            } else {
                Self::estimate_tokens(system) + Self::estimate_tokens(user)
            },
            tokens_out: if usage_data.completion_tokens > 0 {
                usage_data.completion_tokens
            } else {
                Self::estimate_tokens(&text)
            },
        };
        Ok((text, usage))
    }

    async fn call_claude(
        &self,
        settings: &AppSettings,
        system: &str,
        user: &str,
    ) -> Result<(String, AiUsage), String> {
        let api_key = settings.cloud_api_key.as_deref().ok_or("Anthropic API key missing")?;
        let model = settings
            .cloud_model
            .as_deref()
            .unwrap_or("claude-haiku-4-5-20251001");

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&ClaudeRequest {
                model: model.to_string(),
                max_tokens: 1024,
                system: system.to_string(),
                messages: vec![ChatMessage { role: "user".into(), content: user.to_string() }],
            })
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let err = response.text().await.unwrap_or_else(|_| "Unknown error".into());
            return Err(format!("Anthropic API error: {}", err));
        }

        let res: ClaudeResponse = response.json().await.map_err(|e| e.to_string())?;
        let usage_data = res.usage.unwrap_or_default();
        let text = res
            .content
            .into_iter()
            .find(|c| c.content_type == "text")
            .and_then(|c| c.text)
            .ok_or("Empty response from Claude")?;

        let usage = AiUsage {
            tokens_in: if usage_data.input_tokens > 0 {
                usage_data.input_tokens
            } else {
                Self::estimate_tokens(system) + Self::estimate_tokens(user)
            },
            tokens_out: if usage_data.output_tokens > 0 {
                usage_data.output_tokens
            } else {
                Self::estimate_tokens(&text)
            },
        };
        Ok((text, usage))
    }

    // ── Utilities ────────────────────────────────────────────────────────────

    fn clean_sql(&self, input: &str) -> String {
        // Extract from ```sql ... ``` block
        if let Some(start) = input.find("```sql") {
            if let Some(end) = input[start + 6..].find("```") {
                return input[start + 6..start + 6 + end].trim().to_string();
            }
        }
        // Fallback: plain ``` ... ```
        if let Some(start) = input.find("```") {
            if let Some(end) = input[start + 3..].find("```") {
                return input[start + 3..start + 3 + end].trim().to_string();
            }
        }
        input.trim().to_string()
    }
}
