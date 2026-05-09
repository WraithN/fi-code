// MIT License
// Copyright (c) 2025 fi-code contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use super::{AIClient, AnthropicClient, OpenAiClient};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crate::config::{Config, ProviderConfig};

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
    headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct Provider {
    model: Option<Model>,
    http_client: reqwest::Client,
}

impl Default for Provider {
    fn default() -> Self {
        Self {
            model: None,
            http_client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(30))
                .timeout(Duration::from_secs(180))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }
}

impl Provider {
    pub fn new(config: Arc<RwLock<Config>>) -> Result<Self> {
        // 复用同一个 reqwest::Client，避免每次请求都重新建立 TCP/TLS 连接
        // 配置连接和请求超时，避免网络卡顿导致无限等待
        let http_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(180))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        // 1. 优先尝试环境变量
        if let Ok(model) = Self::from_env() {
            return Ok(Self {
                model: Some(model),
                http_client,
            });
        }

        // 2. 降级到配置文件
        let cfg = config.read().map_err(|_| anyhow!("配置锁中毒"))?;
        let model = Self::from_config(&cfg)?;
        Ok(Self {
            model: Some(model),
            http_client,
        })
    }

    fn from_env() -> Result<Model> {
        dotenvy::dotenv().ok();

        // OpenAI
        if let (Ok(api_key), Ok(base_url), Ok(model_name)) = (
            env::var("OPENAI_API_KEY"),
            env::var("OPENAI_BASE_URL"),
            env::var("OPENAI_MODEL_NAME"),
        ) {
            return Ok(Model {
                api_key,
                base_url,
                model_name,
                model_type: ModelType::OpenAiCompatible,
                headers: None,
            });
        }

        // Anthropic
        let anthropic_api_key =
            env::var("ANTHROPIC_API_KEY").or_else(|_| env::var("ANTHROPIC_AUTH_TOKEN"));
        if let (Ok(api_key), Ok(base_url), Ok(model_name)) = (
            anthropic_api_key,
            env::var("ANTHROPIC_BASE_URL"),
            env::var("ANTHROPIC_MODEL_NAME"),
        ) {
            return Ok(Model {
                api_key,
                base_url,
                model_name,
                model_type: ModelType::Anthropic,
                headers: None,
            });
        }

        // GLM (智谱)
        if let (Ok(api_key), Ok(base_url), Ok(model_name)) = (
            env::var("GLM_API_KEY"),
            env::var("GLM_BASE_URL"),
            env::var("GLM_MODEL_NAME"),
        ) {
            return Ok(Model {
                api_key,
                base_url,
                model_name,
                model_type: ModelType::OpenAiCompatible,
                headers: None,
            });
        }

        // Kimi (Moonshot)
        if let (Ok(api_key), Ok(base_url), Ok(model_name)) = (
            env::var("KIMI_API_KEY"),
            env::var("KIMI_BASE_URL"),
            env::var("KIMI_MODEL_NAME"),
        ) {
            return Ok(Model {
                api_key,
                base_url,
                model_name,
                model_type: ModelType::OpenAiCompatible,
                headers: None,
            });
        }

        // DeepSeek
        if let (Ok(api_key), Ok(base_url), Ok(model_name)) = (
            env::var("DEEPSEEK_API_KEY"),
            env::var("DEEPSEEK_BASE_URL"),
            env::var("DEEPSEEK_MODEL_NAME"),
        ) {
            return Ok(Model {
                api_key,
                base_url,
                model_name,
                model_type: ModelType::OpenAiCompatible,
                headers: None,
            });
        }

        // Qwen (通义千问)
        let qwen_api_key = env::var("QWEN_API_KEY").or_else(|_| env::var("DASHSCOPE_API_KEY"));
        if let (Ok(api_key), Ok(base_url), Ok(model_name)) = (
            qwen_api_key,
            env::var("QWEN_BASE_URL"),
            env::var("QWEN_MODEL_NAME"),
        ) {
            return Ok(Model {
                api_key,
                base_url,
                model_name,
                model_type: ModelType::OpenAiCompatible,
                headers: None,
            });
        }

        Err(anyhow!(
            "未找到环境变量配置。请设置 OPENAI_API_KEY、ANTHROPIC_API_KEY、GLM_API_KEY、KIMI_API_KEY 或 QWEN_API_KEY。"
        ))
    }

    pub(crate) fn from_config(config: &Config) -> Result<Model> {
        if let Some((provider_name, model_name, provider_cfg)) =
            Self::resolve_model_ref(&config.model, config)
        {
            let model_type = match provider_cfg.provider_type {
                crate::config::models::ProviderType::Anthropic => ModelType::Anthropic,
                crate::config::models::ProviderType::OpenAiCompatible => {
                    ModelType::OpenAiCompatible
                }
            };
            return Ok(Model {
                api_key: provider_cfg.options.api_key.clone(),
                base_url: provider_cfg.options.base_url.clone(),
                model_name: model_name.to_string(),
                model_type,
                headers: provider_cfg.options.headers.clone(),
            });
        }

        Err(anyhow!("默认模型 '{}' 在配置中未找到", config.model))
    }

    /// 解析模型引用字符串，支持两种格式：
    /// - "provider_key/model_key"：按指定 Provider 查找
    /// - "model_key"：遍历所有 Provider 查找第一个匹配的模型
    ///
    /// 返回：(provider_name, model_name, &provider_cfg)
    fn resolve_model_ref<'a>(
        model_ref: &str,
        config: &'a Config,
    ) -> Option<(String, String, &'a ProviderConfig)> {
        // 尝试 "provider/model" 格式
        if let Some((provider_name, model_name)) = model_ref.split_once('/') {
            let provider_cfg = config.provider.get(provider_name)?;
            if provider_cfg.models.contains_key(model_name) {
                return Some((
                    provider_name.to_string(),
                    model_name.to_string(),
                    provider_cfg,
                ));
            }
            return None;
        }

        // 回退到纯 model_key 遍历查找
        for (provider_name, provider_cfg) in &config.provider {
            if provider_cfg.models.contains_key(model_ref) {
                return Some((provider_name.clone(), model_ref.to_string(), provider_cfg));
            }
        }
        None
    }

    pub fn model_name(&self) -> Result<&str> {
        self.model
            .as_ref()
            .map(|m| m.model_name.as_str())
            .ok_or_else(|| anyhow!("Model not set"))
    }

    /// 运行时切换模型。
    pub fn set_model(&mut self, model_name: &str, config: &Config) -> Result<()> {
        self.set_model_with_key(model_name, config, None)
    }

    /// 运行时切换模型，支持 api_key 覆盖。
    /// model_name 支持 "provider/model" 格式或纯 "model_key"。
    pub fn set_model_with_key(
        &mut self,
        model_name: &str,
        config: &Config,
        api_key_override: Option<&str>,
    ) -> Result<()> {
        if let Some((_provider_name, model_key, provider_cfg)) =
            Self::resolve_model_ref(model_name, config)
        {
            let model_type = match provider_cfg.provider_type {
                crate::config::models::ProviderType::Anthropic => ModelType::Anthropic,
                crate::config::models::ProviderType::OpenAiCompatible => {
                    ModelType::OpenAiCompatible
                }
            };
            self.model = Some(Model {
                api_key: api_key_override
                    .map(|k| k.to_string())
                    .unwrap_or_else(|| provider_cfg.options.api_key.clone()),
                base_url: provider_cfg.options.base_url.clone(),
                model_name: model_key,
                model_type,
                headers: provider_cfg.options.headers.clone(),
            });
            return Ok(());
        }
        Err(anyhow!("模型 '{}' 在配置中未找到", model_name))
    }

    /// 按 Provider + 模型名切换，避免同名模型冲突。
    pub fn set_model_by_provider(
        &mut self,
        provider_name: &str,
        model_name: &str,
        config: &Config,
        api_key_override: Option<&str>,
    ) -> Result<()> {
        let provider_cfg = config
            .provider
            .get(provider_name)
            .ok_or_else(|| anyhow!("Provider '{}' 在配置中未找到", provider_name))?;
        if !provider_cfg.models.contains_key(model_name) {
            return Err(anyhow!(
                "模型 '{}' 在 Provider '{}' 中未找到",
                model_name,
                provider_name
            ));
        }
        let model_type = match provider_cfg.provider_type {
            crate::config::models::ProviderType::Anthropic => ModelType::Anthropic,
            crate::config::models::ProviderType::OpenAiCompatible => ModelType::OpenAiCompatible,
        };
        self.model = Some(Model {
            api_key: api_key_override
                .map(|k| k.to_string())
                .unwrap_or_else(|| provider_cfg.options.api_key.clone()),
            base_url: provider_cfg.options.base_url.clone(),
            model_name: model_name.to_string(),
            model_type,
            headers: provider_cfg.options.headers.clone(),
        });
        Ok(())
    }

    /// 枚举配置中所有可用模型。
    pub fn list_models(&self, config: &Config) -> Vec<(String, String)> {
        let mut result = Vec::new();
        for (_provider_name, provider_cfg) in &config.provider {
            for (model_key, model_cfg) in &provider_cfg.models {
                result.push((model_key.clone(), model_cfg.name.clone()));
            }
        }
        result
    }

    pub fn get_client(&self) -> Result<Box<dyn AIClient>> {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| anyhow!("Model not set"))?;
        if model.model_type == ModelType::OpenAiCompatible {
            let client = OpenAiClient::new(
                self.http_client.clone(),
                model.api_key.clone(),
                model.base_url.clone(),
                model.model_name.clone(),
                model.headers.clone(),
            )?;
            Ok(Box::new(client))
        } else if model.model_type == ModelType::Anthropic {
            let client = AnthropicClient::new(
                self.http_client.clone(),
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
