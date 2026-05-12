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

use std::collections::HashMap;

use super::models::{
    Config, ModelConfig, ModelLimits, ProviderConfig, ProviderOptions, ProviderType,
};

/// 返回所有预设 Provider 的默认配置
/// 用户 config 可以覆盖这些默认值
pub fn default_providers() -> HashMap<String, ProviderConfig> {
    let mut providers = HashMap::new();

    // OpenAI
    providers.insert(
        "openai".to_string(),
        ProviderConfig {
            provider_type: ProviderType::OpenAiCompatible,
            npm: "@ai-sdk/openai".to_string(),
            name: "OpenAI".to_string(),
            options: ProviderOptions {
                api_key: String::new(),
                base_url: "https://api.openai.com/v1".to_string(),
                timeout: 300000,
                chunk_timeout: 10000,
                headers: None,
            },
            models: {
                let mut m = HashMap::new();
                m.insert(
                    "gpt-4o".to_string(),
                    ModelConfig {
                        name: "GPT-4o".to_string(),
                        limit: Some(ModelLimits {
                            context: 128000,
                            output: 16384,
                        }),
                        ..Default::default()
                    },
                );
                m.insert(
                    "gpt-4o-mini".to_string(),
                    ModelConfig {
                        name: "GPT-4o Mini".to_string(),
                        limit: Some(ModelLimits {
                            context: 128000,
                            output: 16384,
                        }),
                        ..Default::default()
                    },
                );
                m.insert(
                    "gpt-4-turbo".to_string(),
                    ModelConfig {
                        name: "GPT-4 Turbo".to_string(),
                        limit: Some(ModelLimits {
                            context: 128000,
                            output: 4096,
                        }),
                        ..Default::default()
                    },
                );
                m
            },
        },
    );

    // GLM (智谱 AI)
    providers.insert(
        "glm".to_string(),
        ProviderConfig {
            provider_type: ProviderType::OpenAiCompatible,
            npm: "@ai-sdk/openai-compatible".to_string(),
            name: "智谱 GLM".to_string(),
            options: ProviderOptions {
                api_key: String::new(),
                base_url: "https://open.bigmodel.cn/api/paas/v4".to_string(),
                timeout: 300000,
                chunk_timeout: 10000,
                headers: None,
            },
            models: {
                let mut m = HashMap::new();
                m.insert(
                    "glm-4".to_string(),
                    ModelConfig {
                        name: "GLM-4".to_string(),
                        limit: Some(ModelLimits {
                            context: 128000,
                            output: 4096,
                        }),
                        ..Default::default()
                    },
                );
                m.insert(
                    "glm-4-flash".to_string(),
                    ModelConfig {
                        name: "GLM-4 Flash".to_string(),
                        limit: Some(ModelLimits {
                            context: 128000,
                            output: 4096,
                        }),
                        ..Default::default()
                    },
                );
                m.insert(
                    "glm-4v".to_string(),
                    ModelConfig {
                        name: "GLM-4V".to_string(),
                        limit: Some(ModelLimits {
                            context: 8192,
                            output: 4096,
                        }),
                        ..Default::default()
                    },
                );
                m
            },
        },
    );

    // Kimi (Moonshot)
    providers.insert(
        "kimi".to_string(),
        ProviderConfig {
            provider_type: ProviderType::OpenAiCompatible,
            npm: "@ai-sdk/openai-compatible".to_string(),
            name: "Moonshot".to_string(),
            options: ProviderOptions {
                api_key: String::new(),
                base_url: "https://api.moonshot.cn/v1".to_string(),
                timeout: 300000,
                chunk_timeout: 10000,
                headers: None,
            },
            models: {
                let mut m = HashMap::new();
                m.insert(
                    "moonshot-v1-8k".to_string(),
                    ModelConfig {
                        name: "Moonshot v1 8K".to_string(),
                        limit: Some(ModelLimits {
                            context: 8192,
                            output: 4096,
                        }),
                        ..Default::default()
                    },
                );
                m.insert(
                    "moonshot-v1-32k".to_string(),
                    ModelConfig {
                        name: "Moonshot v1 32K".to_string(),
                        limit: Some(ModelLimits {
                            context: 32768,
                            output: 4096,
                        }),
                        ..Default::default()
                    },
                );
                m.insert(
                    "moonshot-v1-128k".to_string(),
                    ModelConfig {
                        name: "Moonshot v1 128K".to_string(),
                        limit: Some(ModelLimits {
                            context: 128000,
                            output: 4096,
                        }),
                        ..Default::default()
                    },
                );
                m
            },
        },
    );

    // Qwen (通义千问)
    providers.insert(
        "qwen".to_string(),
        ProviderConfig {
            provider_type: ProviderType::OpenAiCompatible,
            npm: "@ai-sdk/openai-compatible".to_string(),
            name: "通义千问".to_string(),
            options: ProviderOptions {
                api_key: String::new(),
                base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
                timeout: 300000,
                chunk_timeout: 10000,
                headers: None,
            },
            models: {
                let mut m = HashMap::new();
                m.insert(
                    "qwen-turbo".to_string(),
                    ModelConfig {
                        name: "Qwen Turbo".to_string(),
                        limit: Some(ModelLimits {
                            context: 128000,
                            output: 4096,
                        }),
                        ..Default::default()
                    },
                );
                m.insert(
                    "qwen-plus".to_string(),
                    ModelConfig {
                        name: "Qwen Plus".to_string(),
                        limit: Some(ModelLimits {
                            context: 128000,
                            output: 4096,
                        }),
                        ..Default::default()
                    },
                );
                m.insert(
                    "qwen-max".to_string(),
                    ModelConfig {
                        name: "Qwen Max".to_string(),
                        limit: Some(ModelLimits {
                            context: 32000,
                            output: 8192,
                        }),
                        ..Default::default()
                    },
                );
                m
            },
        },
    );

    // DeepSeek
    providers.insert(
        "deepseek".to_string(),
        ProviderConfig {
            provider_type: ProviderType::OpenAiCompatible,
            npm: "@ai-sdk/openai-compatible".to_string(),
            name: "DeepSeek".to_string(),
            options: ProviderOptions {
                api_key: String::new(),
                base_url: "https://api.deepseek.com".to_string(),
                timeout: 300000,
                chunk_timeout: 10000,
                headers: None,
            },
            models: {
                let mut m = HashMap::new();
                m.insert(
                    "deepseek-chat".to_string(),
                    ModelConfig {
                        name: "DeepSeek-V3".to_string(),
                        limit: Some(ModelLimits {
                            context: 64000,
                            output: 8192,
                        }),
                        ..Default::default()
                    },
                );
                m.insert(
                    "deepseek-reasoner".to_string(),
                    ModelConfig {
                        name: "DeepSeek-R1".to_string(),
                        limit: Some(ModelLimits {
                            context: 64000,
                            output: 8192,
                        }),
                        ..Default::default()
                    },
                );
                m
            },
        },
    );

    // Anthropic
    providers.insert(
        "anthropic".to_string(),
        ProviderConfig {
            provider_type: ProviderType::Anthropic,
            npm: "@ai-sdk/anthropic".to_string(),
            name: "Anthropic".to_string(),
            options: ProviderOptions {
                api_key: String::new(),
                base_url: "https://api.anthropic.com".to_string(),
                timeout: 300000,
                chunk_timeout: 10000,
                headers: None,
            },
            models: {
                let mut m = HashMap::new();
                m.insert(
                    "claude-3-7-sonnet-20250219".to_string(),
                    ModelConfig {
                        name: "Claude 3.7 Sonnet".to_string(),
                        limit: Some(ModelLimits {
                            context: 200000,
                            output: 8192,
                        }),
                        ..Default::default()
                    },
                );
                m.insert(
                    "claude-3-5-sonnet-20241022".to_string(),
                    ModelConfig {
                        name: "Claude 3.5 Sonnet".to_string(),
                        limit: Some(ModelLimits {
                            context: 200000,
                            output: 8192,
                        }),
                        ..Default::default()
                    },
                );
                m.insert(
                    "claude-3-opus-20240229".to_string(),
                    ModelConfig {
                        name: "Claude 3 Opus".to_string(),
                        limit: Some(ModelLimits {
                            context: 200000,
                            output: 4096,
                        }),
                        ..Default::default()
                    },
                );
                m
            },
        },
    );

    providers
}

/// 将预设 Provider 合并到用户配置中
/// 用户配置覆盖预设值，自定义 Provider 保留
pub fn merge_presets(config: &mut Config) {
    let presets = default_providers();
    for (key, preset) in presets {
        if !config.provider.contains_key(&key) {
            config.provider.insert(key, preset);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_providers_includes_openai() {
        let providers = default_providers();
        assert!(providers.contains_key("openai"));
        assert!(providers.contains_key("glm"));
        assert!(providers.contains_key("kimi"));
        assert!(providers.contains_key("qwen"));
        assert!(providers.contains_key("anthropic"));
    }

    #[test]
    fn test_merge_presets_adds_missing() {
        let mut config = Config {
            model: "gpt-4o".to_string(),
            provider: HashMap::new(),
            mcp: None,
            server: None,
            source_path: None,
        };
        merge_presets(&mut config);
        assert!(config.provider.contains_key("openai"));
        assert!(config.provider.contains_key("kimi"));
    }

    #[test]
    fn test_merge_presets_preserves_user_override() {
        let mut config = Config {
            model: "gpt-4o".to_string(),
            provider: {
                let mut p = HashMap::new();
                p.insert(
                    "openai".to_string(),
                    ProviderConfig {
                        provider_type: ProviderType::OpenAiCompatible,
                        npm: "custom".to_string(),
                        name: "Custom OpenAI".to_string(),
                        options: ProviderOptions {
                            api_key: "sk-test".to_string(),
                            base_url: "https://custom.com".to_string(),
                            timeout: 1000,
                            chunk_timeout: 1000,
                            headers: None,
                        },
                        models: HashMap::new(),
                    },
                );
                p
            },
            mcp: None,
            server: None,
            source_path: None,
        };
        merge_presets(&mut config);
        let openai = config.provider.get("openai").unwrap();
        assert_eq!(openai.name, "Custom OpenAI");
        assert_eq!(openai.options.base_url, "https://custom.com");
    }

    #[test]
    fn test_merge_presets_preserves_custom_provider() {
        let mut config = Config {
            model: "gpt-4o".to_string(),
            provider: {
                let mut p = HashMap::new();
                p.insert(
                    "my-custom".to_string(),
                    ProviderConfig {
                        provider_type: ProviderType::OpenAiCompatible,
                        npm: "custom".to_string(),
                        name: "My Custom".to_string(),
                        options: ProviderOptions {
                            api_key: String::new(),
                            base_url: "https://custom.com".to_string(),
                            timeout: 300000,
                            chunk_timeout: 10000,
                            headers: None,
                        },
                        models: HashMap::new(),
                    },
                );
                p
            },
            mcp: None,
            server: None,
            source_path: None,
        };
        merge_presets(&mut config);
        assert!(config.provider.contains_key("my-custom"));
        assert!(config.provider.contains_key("openai"));
    }
}
