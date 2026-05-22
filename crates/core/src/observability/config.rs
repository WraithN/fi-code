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

// 可观测性配置解析：合并 env 与 config.json，输出运行时可直接使用的 ObservabilityConfig
// 优先级：环境变量 > config.json > 默认值
// 当且仅当 enabled = true 且 public_key、secret_key 同时存在时，才视为已启用

use crate::config::models::Config;

// Langfuse 平台默认接入域名
const DEFAULT_LANGFUSE_HOST: &str = "https://cloud.langfuse.com";

// 各环境变量名集中常量定义，避免拼写错误
const ENV_LANGFUSE_HOST: &str = "LANGFUSE_HOST";
const ENV_LANGFUSE_PUBLIC_KEY: &str = "LANGFUSE_PUBLIC_KEY";
const ENV_LANGFUSE_SECRET_KEY: &str = "LANGFUSE_SECRET_KEY";
const ENV_LANGFUSE_ENVIRONMENT: &str = "LANGFUSE_ENVIRONMENT";
const ENV_LANGFUSE_RELEASE: &str = "LANGFUSE_RELEASE";

/// 运行时可观测性配置聚合体
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ObservabilityConfig {
    pub langfuse: LangfuseConfig,
}

/// Langfuse 实际生效配置
#[derive(Debug, Clone, PartialEq)]
pub struct LangfuseConfig {
    pub enabled: bool,
    pub host: String,
    pub public_key: Option<String>,
    pub secret_key: Option<String>,
    pub environment: Option<String>,
    pub release: Option<String>,
}

impl Default for LangfuseConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: DEFAULT_LANGFUSE_HOST.to_string(),
            public_key: None,
            secret_key: None,
            environment: None,
            release: None,
        }
    }
}

impl ObservabilityConfig {
    /// 从 fi-code 主配置中解析出可观测性配置。
    /// 合并策略：env > config.json > 默认值
    pub fn resolve(config: &Config) -> Self {
        let raw_langfuse = config
            .observability
            .as_ref()
            .and_then(|o| o.langfuse.as_ref());

        // 逐字段读取 env，若不存在则回落到 config 中的值
        let host = read_env(ENV_LANGFUSE_HOST)
            .or_else(|| raw_langfuse.and_then(|l| l.host.clone()))
            .unwrap_or_else(|| DEFAULT_LANGFUSE_HOST.to_string());

        let public_key = read_env(ENV_LANGFUSE_PUBLIC_KEY)
            .or_else(|| raw_langfuse.and_then(|l| l.public_key.clone()));

        let secret_key = read_env(ENV_LANGFUSE_SECRET_KEY)
            .or_else(|| raw_langfuse.and_then(|l| l.secret_key.clone()));

        let environment = read_env(ENV_LANGFUSE_ENVIRONMENT)
            .or_else(|| raw_langfuse.and_then(|l| l.environment.clone()));

        let release = read_env(ENV_LANGFUSE_RELEASE)
            .or_else(|| raw_langfuse.and_then(|l| l.release.clone()));

        // config 中显式 enabled=true（缺省视为 true，让 key 是否存在决定）
        // 经典语义：只要两把 key 都有，并且没有显式 disable，就算启用
        let config_enabled = raw_langfuse
            .and_then(|l| l.enabled)
            .unwrap_or(true);

        let enabled = config_enabled && public_key.is_some() && secret_key.is_some();

        Self {
            langfuse: LangfuseConfig {
                enabled,
                host,
                public_key,
                secret_key,
                environment,
                release,
            },
        }
    }

    /// 便捷查询：当前是否启用 Langfuse 上报
    pub fn langfuse_enabled(&self) -> bool {
        self.langfuse.enabled
    }
}

// 读取环境变量，忽略空字符串
fn read_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::{LangfuseRawConfig, ObservabilityRawConfig};
    use std::sync::Mutex;

    // env 是进程级共享资源，cargo test 默认并行运行；用互斥锁串行化所有依赖 env 的用例
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // 测试隔离：每个用例必须清理所有相关 env，避免相互污染
    fn clear_env() {
        for k in &[
            ENV_LANGFUSE_HOST,
            ENV_LANGFUSE_PUBLIC_KEY,
            ENV_LANGFUSE_SECRET_KEY,
            ENV_LANGFUSE_ENVIRONMENT,
            ENV_LANGFUSE_RELEASE,
        ] {
            std::env::remove_var(k);
        }
    }

    #[test]
    fn test_disabled_when_no_keys() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        let cfg = Config::default();
        let obs = ObservabilityConfig::resolve(&cfg);
        assert!(!obs.langfuse_enabled());
        assert_eq!(obs.langfuse.host, DEFAULT_LANGFUSE_HOST);
        assert!(obs.langfuse.public_key.is_none());
        assert!(obs.langfuse.secret_key.is_none());
    }

    #[test]
    fn test_enabled_via_env_only() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        std::env::set_var(ENV_LANGFUSE_PUBLIC_KEY, "pk-lf-env");
        std::env::set_var(ENV_LANGFUSE_SECRET_KEY, "sk-lf-env");

        let cfg = Config::default();
        let obs = ObservabilityConfig::resolve(&cfg);

        assert!(obs.langfuse_enabled(), "两把 key 都通过 env 提供时应启用");
        assert_eq!(obs.langfuse.public_key.as_deref(), Some("pk-lf-env"));
        assert_eq!(obs.langfuse.secret_key.as_deref(), Some("sk-lf-env"));
        // 未指定 host，应使用默认值
        assert_eq!(obs.langfuse.host, DEFAULT_LANGFUSE_HOST);

        clear_env();
    }

    #[test]
    fn test_env_overrides_config_host() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        // config.json 提供一个 host
        let cfg = Config {
            observability: Some(ObservabilityRawConfig {
                langfuse: Some(LangfuseRawConfig {
                    enabled: Some(true),
                    host: Some("https://config-host.example.com".to_string()),
                    public_key: Some("pk-cfg".to_string()),
                    secret_key: Some("sk-cfg".to_string()),
                    environment: Some("staging".to_string()),
                    release: None,
                }),
            }),
            ..Default::default()
        };

        // env 中给出另一个 host
        std::env::set_var(ENV_LANGFUSE_HOST, "https://env-host.example.com");

        let obs = ObservabilityConfig::resolve(&cfg);
        assert_eq!(obs.langfuse.host, "https://env-host.example.com");
        // env 未覆盖 key 与 environment，应保留 config 值
        assert_eq!(obs.langfuse.public_key.as_deref(), Some("pk-cfg"));
        assert_eq!(obs.langfuse.secret_key.as_deref(), Some("sk-cfg"));
        assert_eq!(obs.langfuse.environment.as_deref(), Some("staging"));
        assert!(obs.langfuse_enabled());

        clear_env();
    }
}
