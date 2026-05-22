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

use super::models::{Config, ProviderConfig};

/// 预设 Provider 配置，编译时从 JSON 文件嵌入二进制。
const PRESET_MODELS_JSON: &str = include_str!("preset_models.json");

/// 返回所有预设 Provider 的默认配置
/// 用户 config 可以覆盖这些默认值
pub fn default_providers() -> HashMap<String, ProviderConfig> {
    serde_json::from_str(PRESET_MODELS_JSON)
        .expect("preset_models.json 格式错误，必须是有效的 ProviderConfig Map")
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
            observability: None,
            source_path: None,
        };
        merge_presets(&mut config);
        assert!(config.provider.contains_key("openai"));
        assert!(config.provider.contains_key("kimi"));
    }

    #[test]
    fn test_merge_presets_preserves_user_override() {
        use super::super::models::{ProviderOptions, ProviderType};
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
            observability: None,
            source_path: None,
        };
        merge_presets(&mut config);
        let openai = config.provider.get("openai").unwrap();
        assert_eq!(openai.name, "Custom OpenAI");
        assert_eq!(openai.options.base_url, "https://custom.com");
    }

    #[test]
    fn test_merge_presets_preserves_custom_provider() {
        use super::super::models::{ProviderOptions, ProviderType};
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
            observability: None,
            source_path: None,
        };
        merge_presets(&mut config);
        assert!(config.provider.contains_key("my-custom"));
        assert!(config.provider.contains_key("openai"));
    }
}
