//! Language model calls: the local llama.cpp server by default, or the user's own
//! OpenAI-compatible or Anthropic key. Supports streaming for chat-style answers.

use crate::stt::CloudCfg;
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    #[default]
    Local,
    /// Any OpenAI-compatible API (OpenAI, Groq, OpenRouter, a self-hosted server…).
    OpenAi,
    Anthropic,
}

#[derive(Clone, Debug)]
pub struct Msg {
    pub role: &'static str,
    pub content: String,
}

pub fn msg(role: &'static str, content: impl Into<String>) -> Msg {
    Msg { role, content: content.into() }
}

#[derive(Clone)]
pub struct Llm {
    pub provider: Provider,
    pub local_base: Option<String>,
    pub cloud: CloudCfg,
    pub temperature: f32,
}

impl Llm {
    pub async fn chat(&self, system: &str, msgs: &[Msg], max_tokens: u32, timeout: Duration) -> Result<String> {
        self.run(system, msgs, max_tokens, timeout, None).await
    }

    /// Like `chat`, calling `on_text` with each new piece of the answer as it arrives.
    pub async fn chat_stream(&self, system: &str, msgs: &[Msg], max_tokens: u32, timeout: Duration, on_text: &(dyn Fn(&str) + Send + Sync)) -> Result<String> {
        self.run(system, msgs, max_tokens, timeout, Some(on_text)).await
    }

    async fn run(&self, system: &str, msgs: &[Msg], max_tokens: u32, timeout: Duration, on_text: Option<&(dyn Fn(&str) + Send + Sync)>) -> Result<String> {
        let client = reqwest::Client::new();
        let stream = on_text.is_some();
        let req = match self.provider {
            Provider::Anthropic => {
                let body = json!({
                    "model": self.cloud.model,
                    "max_tokens": max_tokens,
                    "system": system,
                    "temperature": self.temperature,
                    "stream": stream,
                    "messages": msgs.iter().map(|m| json!({"role": m.role, "content": m.content})).collect::<Vec<_>>(),
                });
                let base = if self.cloud.base_url.trim().is_empty() { "https://api.anthropic.com" } else { self.cloud.base_url.trim_end_matches('/') };
                client.post(format!("{base}/v1/messages")).header("x-api-key", &self.cloud.api_key).header("anthropic-version", "2023-06-01").json(&body)
            }
            _ => {
                let (base, key, model) = match self.provider {
                    Provider::Local => (format!("{}/v1", self.local_base.clone().ok_or_else(|| anyhow!("the local AI model is not running"))?), String::new(), "local".to_string()),
                    _ => (self.cloud.base_url.trim_end_matches('/').to_string(), self.cloud.api_key.clone(), self.cloud.model.clone()),
                };
                let mut messages = vec![json!({"role": "system", "content": system})];
                messages.extend(msgs.iter().map(|m| json!({"role": m.role, "content": m.content})));
                let mut body = json!({ "model": model, "messages": messages, "temperature": self.temperature, "max_tokens": max_tokens, "stream": stream });
                if self.provider == Provider::Local {
                    // Qwen3 hybrid models: answer directly, no thinking block.
                    body["chat_template_kwargs"] = json!({"enable_thinking": false});
                    body["cache_prompt"] = json!(true);
                }
                let mut r = client.post(format!("{base}/chat/completions")).json(&body);
                if !key.is_empty() {
                    r = r.bearer_auth(key);
                }
                r
            }
        };
        let resp = req.timeout(timeout).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let v: Value = resp.json().await.unwrap_or_default();
            let m = v["error"]["message"].as_str().or(v["error"].as_str()).unwrap_or("request failed").to_string();
            return Err(anyhow!("AI model {}: {m}", status.as_u16()));
        }
        let out = if let Some(cb) = on_text {
            let mut out = String::new();
            let mut buf = String::new();
            let mut s = resp.bytes_stream();
            while let Some(chunk) = s.next().await {
                buf.push_str(&String::from_utf8_lossy(&chunk?));
                while let Some(nl) = buf.find('\n') {
                    let line = buf[..nl].trim().to_string();
                    buf.drain(..=nl);
                    let Some(data) = line.strip_prefix("data:") else { continue };
                    let data = data.trim();
                    if data == "[DONE]" {
                        continue;
                    }
                    let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
                    let piece = if self.provider == Provider::Anthropic {
                        v["delta"]["text"].as_str().unwrap_or("").to_string()
                    } else {
                        v["choices"][0]["delta"]["content"].as_str().unwrap_or("").to_string()
                    };
                    if !piece.is_empty() {
                        out.push_str(&piece);
                        cb(&piece);
                    }
                }
            }
            out
        } else {
            let v: Value = resp.json().await?;
            if self.provider == Provider::Anthropic {
                v["content"].as_array().map(|a| a.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("")).unwrap_or_default()
            } else {
                v["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string()
            }
        };
        Ok(strip_reasoning(&out))
    }
}

pub fn strip_reasoning(s: &str) -> String {
    let mut t = s.to_string();
    while let (Some(a), Some(b)) = (t.find("<think>"), t.find("</think>")) {
        if b < a {
            break;
        }
        t.replace_range(a..b + "</think>".len(), "");
    }
    let t = t.trim();
    let t = if t.starts_with("```") && t.ends_with("```") && t.len() > 6 {
        t.trim_start_matches("```").trim_end_matches("```").trim_start_matches(|c: char| c.is_alphanumeric()).trim()
    } else {
        t
    };
    t.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n")
}

/// Rough token count for budgeting prompts (about 4 characters per token in English).
pub fn approx_tokens(s: &str) -> usize {
    s.len() / 4 + 1
}

#[cfg(test)]
mod tests {
    #[test]
    fn strip() {
        assert_eq!(super::strip_reasoning("<think>x</think>\nHello"), "Hello");
        assert_eq!(super::strip_reasoning("```markdown\n# Hi\n```"), "# Hi");
    }
}
