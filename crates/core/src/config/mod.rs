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

pub mod config;
pub mod models;
pub mod presets;

pub use models::{
    Config, McpServerConfig, McpServerType, ModelConfig, ModelLimits, ProviderConfig,
    ProviderOptions, ProviderType,
};
pub use presets::merge_presets;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.model.is_empty());
        assert!(config.provider.is_empty());
    }

    #[test]
    fn test_parse_json_config() {
        let json = r#"{
            "model": "my-model",
            "provider": {
                "openai": {
                    "npm": "@ai-sdk/openai-compatible",
                    "name": "OpenAI",
                    "options": {
                        "apiKey": "sk-test",
                        "baseURL": "https://api.openai.com/v1",
                        "timeout": 300000,
                        "chunkTimeout": 10000
                    },
                    "models": {
                        "my-model": {
                            "name": "My Model",
                            "limit": { "context": 200000, "output": 65536 }
                        }
                    }
                }
            }
        }"#;

        let config = Config::parse(json, false).unwrap();
        assert_eq!(config.model, "my-model");
        assert!(config.provider.contains_key("openai"));
    }

    #[test]
    fn test_parse_jsonc_with_comments() {
        let jsonc = r#"{
            // 默认模型
            "model": "my-model",
            "provider": {
                "openai": {
                    "npm": "@ai-sdk/openai-compatible",
                    "name": "OpenAI",
                    "options": {
                        "apiKey": "sk-test",
                        "baseURL": "https://api.openai.com/v1",
                        "timeout": 300000,
                        "chunkTimeout": 10000
                    },
                    "models": {
                        "my-model": {
                            "name": "My Model",
                            "limit": { "context": 200000, "output": 65536 }
                        }
                    }
                }
            }
        }"#;

        let config = Config::parse(jsonc, true).unwrap();
        assert_eq!(config.model, "my-model");
    }

    #[test]
    fn test_parse_mcp_config() {
        let json = r#"{
            "model": "test-model",
            "provider": {},
            "mcp": {
                "local-server": {
                    "type": "local",
                    "enabled": true,
                    "command": ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
                },
                "remote-server": {
                    "type": "remote",
                    "enabled": false,
                    "url": "http://localhost:3000/mcp",
                    "headers": {"Authorization": "Bearer test"}
                }
            }
        }"#;

        let config = Config::parse(json, false).unwrap();
        let mcp = config.mcp.as_ref().unwrap();
        assert_eq!(mcp.len(), 2);

        let local = mcp.get("local-server").unwrap();
        assert_eq!(local.server_type, McpServerType::Local);
        assert!(local.enabled);
        assert_eq!(local.command.as_ref().unwrap()[0], "npx");

        let remote = mcp.get("remote-server").unwrap();
        assert_eq!(remote.server_type, McpServerType::Remote);
        assert!(!remote.enabled);
        assert_eq!(remote.url.as_ref().unwrap(), "http://localhost:3000/mcp");
    }

    #[test]
    fn test_env_placeholder_resolution() {
        std::env::set_var("TEST_API_KEY", "resolved-key");

        let json = r#"{
            "model": "test-model",
            "provider": {
                "test": {
                    "npm": "test",
                    "name": "Test",
                    "options": {
                        "apiKey": "{env:TEST_API_KEY}",
                        "baseURL": "https://test.com",
                        "timeout": 1000,
                        "chunkTimeout": 1000
                    },
                    "models": {
                        "test-model": {
                            "name": "Test Model",
                            "limit": { "context": 1000, "output": 1000 }
                        }
                    }
                }
            }
        }"#;

        let mut config = Config::parse(json, false).unwrap();
        config.resolve_env_placeholders().unwrap();

        let provider = config.provider.get("test").unwrap();
        assert_eq!(provider.options.api_key, "resolved-key");
    }
}
