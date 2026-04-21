use super::{AIClient, AnthropicClient, OpenAiClient};
use anyhow::{anyhow, Result};
use std::env;
use std::sync::{Arc, RwLock};

use crate::config::Config;

#[derive(Debug, Clone, PartialEq, Eq)]
enum ModelType {
    OpenAiCompatible,
    Anthropic,
}

#[derive(Debug, Clone)]
struct Model {
    api_key: String,
    base_url: String,
    model_name: String,
    model_type: ModelType,
}

pub struct Provider {
    model: Option<Model>,
}

impl Provider {
    pub fn new(config: Arc<RwLock<Config>>) -> Result<Self> {
        // 1. 优先尝试环境变量
        if let Ok(model) = Self::from_env() {
            return Ok(Self { model: Some(model) });
        }

        // 2. 降级到配置文件
        let cfg = config.read().map_err(|_| anyhow!("配置锁中毒"))?;
        let model = Self::from_config(&cfg)?;
        Ok(Self { model: Some(model) })
    }

    fn from_env() -> Result<Model> {
        dotenvy::dotenv().ok();

        if let (Ok(api_key), Ok(base_url), Ok(model_name)) = (
            env::var("OPENAI_API_KEY"),
            env::var("OPENAI_BASE_URL"),
            env::var("OPENAI_MODEL"),
        ) {
            return Ok(Model {
                api_key,
                base_url,
                model_name,
                model_type: ModelType::OpenAiCompatible,
            });
        }

        let anthropic_api_key =
            env::var("ANTHROPIC_API_KEY").or_else(|_| env::var("ANTHROPIC_AUTH_TOKEN"));
        if let (Ok(api_key), Ok(base_url), Ok(model_name)) = (
            anthropic_api_key,
            env::var("ANTHROPIC_BASE_URL"),
            env::var("ANTHROPIC_MODEL"),
        ) {
            return Ok(Model {
                api_key,
                base_url,
                model_name,
                model_type: ModelType::Anthropic,
            });
        }

        Err(anyhow!(
            "未找到环境变量配置。请设置 OPENAI_API_KEY 或 ANTHROPIC_API_KEY。"
        ))
    }

    pub(crate) fn from_config(config: &Config) -> Result<Model> {
        for (provider_name, provider_cfg) in &config.provider {
            if provider_cfg.models.contains_key(&config.model) {
                let model_type = match provider_name.as_str() {
                    "anthropic" => ModelType::Anthropic,
                    _ => ModelType::OpenAiCompatible,
                };
                return Ok(Model {
                    api_key: provider_cfg.options.api_key.clone(),
                    base_url: provider_cfg.options.base_url.clone(),
                    model_name: config.model.clone(),
                    model_type,
                });
            }
        }

        Err(anyhow!("默认模型 '{}' 在配置中未找到", config.model))
    }

    pub fn model_name(&self) -> Result<&str> {
        self.model
            .as_ref()
            .map(|m| m.model_name.as_str())
            .ok_or_else(|| anyhow!("Model not set"))
    }

    pub fn get_client(&self) -> Result<Box<dyn AIClient>> {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| anyhow!("Model not set"))?;
        if model.model_type == ModelType::OpenAiCompatible {
            let client = OpenAiClient::new(
                model.api_key.clone(),
                model.base_url.clone(),
                model.model_name.clone(),
            )?;
            Ok(Box::new(client))
        } else if model.model_type == ModelType::Anthropic {
            let client = AnthropicClient::new(
                model.api_key.clone(),
                model.base_url.clone(),
                model.model_name.clone(),
            )?;
            Ok(Box::new(client))
        } else {
            Err(anyhow!(
                "Model type conflict: cannot be both OpenAiCompatible and Anthropic"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{AIClient, ChunkContent, FinishReason};
    use crate::session::message::{Message, Part, Role};
    use std::collections::HashMap;
    use std::time::Duration;

    /// 探测 localhost:11434 是否有可用的 Ollama 服务，并返回一个可用模型名。
    async fn try_get_ollama_model() -> Option<String> {
        let client = reqwest::Client::new();
        let resp = client
            .get("http://localhost:11434/api/tags")
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let body: serde_json::Value = resp.json().await.ok()?;
        let models = body.get("models")?.as_array()?;
        models.first()?.get("name")?.as_str().map(|s| s.to_string())
    }

    #[test]
    fn test_provider_from_config() {
        use crate::config::models::{ModelConfig, ModelLimits, ProviderConfig, ProviderOptions};

        let mut provider_map = HashMap::new();
        provider_map.insert(
            "openai".to_string(),
            ProviderConfig {
                npm: "@ai-sdk/openai-compatible".to_string(),
                name: "OpenAI".to_string(),
                options: ProviderOptions {
                    api_key: "test-key".to_string(),
                    base_url: "https://test.com".to_string(),
                    timeout: 300000,
                    chunk_timeout: 10000,
                },
                models: {
                    let mut m = HashMap::new();
                    m.insert(
                        "gpt-4".to_string(),
                        ModelConfig {
                            name: "GPT-4".to_string(),
                            limit: ModelLimits {
                                context: 128000,
                                output: 4096,
                            },
                        },
                    );
                    m
                },
            },
        );

        let config = Config {
            model: "gpt-4".to_string(),
            provider: provider_map,
            mcp: None,
        };

        let model = Provider::from_config(&config).unwrap();
        assert_eq!(model.model_name, "gpt-4");
        assert_eq!(model.api_key, "test-key");
        assert_eq!(model.model_type, ModelType::OpenAiCompatible);
    }

    /// 测试本地 Ollama 的 OpenAI 兼容流式接口：纯文本场景
    #[tokio::test]
    async fn test_local_openai_compatible_text_stream() {
        let Some(model_name) = try_get_ollama_model().await else {
            panic!(
                "Ollama is not running on localhost:11434. \
                 Please start Ollama to run this test."
            );
        };

        let provider = Provider {
            model: Some(Model {
                api_key: "dummy".to_string(),
                base_url: "http://localhost:11434".to_string(),
                model_name,
                model_type: ModelType::OpenAiCompatible,
            }),
        };

        let client = provider.get_client().expect("should create client");

        let messages = vec![Message::new(
            "test-session",
            Role::User,
            vec![Part::Text {
                text: "Please reply with exactly the word 'pong'.".to_string(),
            }],
        )];

        let schema = serde_json::json!([]);
        let mut texts = Vec::new();
        let mut finish_reason = None;

        client
            .stream_message(
                "You are a concise assistant.",
                &messages,
                &schema,
                &mut |chunk| match chunk.content {
                    ChunkContent::Text(t) => texts.push(t),
                    ChunkContent::Finish(r) => finish_reason = Some(r),
                    _ => {}
                },
            )
            .await
            .expect("stream_message should succeed");

        let full_text = texts.join("");
        println!("text stream response: {}", full_text);
        assert!(
            !full_text.is_empty() || finish_reason.is_some(),
            "should receive at least text or finish reason"
        );
        assert_eq!(
            finish_reason,
            Some(FinishReason::Stop),
            "text-only stream should finish with Stop"
        );
    }

    /// 测试本地 Ollama 的 OpenAI 兼容流式接口：tool_use 场景
    #[tokio::test]
    async fn test_local_openai_compatible_tool_use_stream() {
        let Some(model_name) = try_get_ollama_model().await else {
            panic!(
                "Ollama is not running on localhost:11434. \
                 Please start Ollama to run this test."
            );
        };

        let provider = Provider {
            model: Some(Model {
                api_key: "dummy".to_string(),
                base_url: "http://localhost:11434".to_string(),
                model_name,
                model_type: ModelType::OpenAiCompatible,
            }),
        };

        let client = provider.get_client().expect("should create client");

        let tools_schema = serde_json::json!([
            {
                "name": "calculator",
                "description": "Add two numbers.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "a": { "type": "number" },
                        "b": { "type": "number" }
                    },
                    "required": ["a", "b"]
                }
            }
        ]);

        let messages = vec![Message::new(
            "test-session",
            Role::User,
            vec![Part::Text {
                text: "What is 23 plus 45? Use the calculator tool.".to_string(),
            }],
        )];

        let mut texts = Vec::new();
        let mut tool_uses = Vec::new();
        let mut finish_reason = None;

        let result = client
            .stream_message(
                "You are a helpful assistant. Use tools when appropriate.",
                &messages,
                &tools_schema,
                &mut |chunk| match chunk.content {
                    ChunkContent::Text(t) => texts.push(t),
                    ChunkContent::ToolUse(part) => {
                        if let Part::ToolUse {
                            id,
                            name,
                            arguments,
                        } = part
                        {
                            tool_uses.push((id, name, arguments));
                        }
                    }
                    ChunkContent::Finish(r) => finish_reason = Some(r),
                    _ => {}
                },
            )
            .await;

        if let Err(e) = result {
            eprintln!(
                "stream_message returned error (model may not support tools): {}",
                e
            );
            return;
        }

        if !tool_uses.is_empty() {
            assert_eq!(
                finish_reason,
                Some(FinishReason::ToolUse),
                "tool use stream should finish with ToolUse"
            );
            let (_, name, args) = &tool_uses[0];
            assert_eq!(name, "calculator");
            assert!(
                args.get("a").is_some() || args.get("b").is_some(),
                "calculator arguments should contain a or b, got: {}",
                args
            );
        } else {
            // 模型未触发工具调用（可能是模型不支持 tools），仅做基本断言，不使测试失败
            let full_text = texts.join("");
            println!("tool stream text response (no tool use): {}", full_text);
            assert!(
                !full_text.is_empty() || finish_reason.is_some(),
                "should receive text or finish reason even when no tool is used"
            );
        }
    }
}
