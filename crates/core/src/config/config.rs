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

// ============================================================================
// config.json / config.jsonc Schema（配置文件结构参考）
// ============================================================================
//
// {
//   // 默认使用的模型 key，格式为 "provider_key/model_key"
//   // 对应下方 provider.<provider_key>.models 中的某个 key
//   "model": "openai/kimi-k2.5",
//
//   // Provider 映射表，key 为 provider 标识符（如 openai、anthropic、glm 等）
//   "provider": {
//     "openai": {
//       // API 类型："openai_compatible" | "anthropic"
//       "provider_type": "openai_compatible",
//
//       // npm 包名（内部使用）
//       "npm": "@ai-sdk/openai-compatible",
//
//       // Provider 显示名称
//       "name": "Volcano Ark Code",
//
//       // 连接选项
//       "options": {
//         // API Key，支持 {env:VAR_NAME} 占位符语法，启动时自动替换为环境变量值
//         "apiKey": "{env:OPENAI_API_KEY}",
//
//         // 基础 URL
//         "baseURL": "https://ark.cn-beijing.volces.com/api/coding/v3",
//
//         // 额外的 HTTP 请求头（可选）
//         "headers": {
//           "Accept": "application/json",
//           "Content-Type": "application/json",
//           "X-Volc-Region": "cn-beijing"
//         },
//
//         // 请求总超时（毫秒）
//         "timeout": 300000,
//
//         // 单个 chunk 超时（毫秒）
//         "chunkTimeout": 10000
//       },
//
//       // 该 Provider 下可用的模型列表
//       "models": {
//         "kimi-k2.5": {
//           "name": "Kimi K2.5",
//
//           // 最大 Token 数（可选）
//           "maxTokens": 128000,
//
//           // 输入/输出模态（可选）
//           "modalities": {
//             "input": ["text", "image"],
//             "output": ["text"]
//           },
//
//           // 模型级别的额外选项（可选，透传给下游 SDK）
//           "options": {
//             "thinking": {
//               "type": "enabled"
//             }
//           }
//         },
//         "ark-code-latest": {
//           "name": "ark-code-latest"
//         }
//       }
//     }
//   },
//
//   // MCP（Model Context Protocol）服务器配置（可选）
//   "mcp": {
//     "filesystem": {
//       // 服务器类型："local" | "remote"
//       "type": "local",
//
//       // 是否启用
//       "enabled": true,
//
//       // local 类型下的启动命令（可选）
//       "command": ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/path"],
//
//       // remote 类型下的 URL（可选）
//       "url": null,
//
//       // 额外的 HTTP 请求头（可选）
//       "headers": null
//     }
//   },
//
//   // 服务端配置（可选）
//   "server": {
//     // 监听端口，默认 4040
//     "port": 4040,
//
//     // API 访问令牌（可选）
//     "api_token": null,
//
//     // 允许的 CORS Origin 列表（可选）
//     "allowed_origins": null
//   }
// }
//
// 特性说明：
// - config.jsonc 支持 // 和 /* */ 注释
// - apiKey 支持 {env:VAR_NAME} 占位符，启动时自动解析
// - 配置文件变更后自动热重载（500ms 防抖）
// - 预设 Provider（openai、anthropic、glm、kimi、qwen、deepseek）会自动合并到配置中
// - 模型字段 limit / maxTokens / modalities / options 均为可选
// ============================================================================

use anyhow::{anyhow, Context, Result};
use notify::Watcher;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use super::models::Config;
use crate::log_info;

impl Config {
    /// 返回配置目录路径：~/.config/fi-code/
    pub fn config_dir() -> PathBuf {
        directories::ProjectDirs::from("", "", "fi-code")
            .map(|d| d.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".config/fi-code"))
    }

    /// 加载配置文件，支持 .jsonc 和 .json
    pub fn load() -> Result<Self> {
        let config_dir = Self::config_dir();
        let paths = [
            config_dir.join("config.jsonc"),
            config_dir.join("config.json"),
        ];

        for path in &paths {
            if path.exists() {
                let content = fs::read_to_string(path)
                    .with_context(|| format!("无法读取配置文件: {:?}", path))?;
                let is_jsonc = path.extension().map(|e| e == "jsonc").unwrap_or(false);
                let mut config = Self::parse(&content, is_jsonc)?;
                config.source_path = Some(path.display().to_string());
                super::presets::merge_presets(&mut config);
                log_info!("[Server] Config loaded from: {}", path.display());
                return Ok(config);
            }
        }

        let mut config = Config::default();
        super::presets::merge_presets(&mut config);
        config.source_path = Some("default".to_string());
        log_info!("[Server] Config loaded: default (no config file found)");
        Ok(config)
    }

    pub fn parse(content: &str, is_jsonc: bool) -> Result<Self> {
        let mut config: Config = if is_jsonc {
            jsonc_parser::parse_to_serde_value(content, &Default::default())
                .map_err(|e| anyhow!("JSONC 解析失败: {}", e))?
        } else {
            serde_json::from_str(content).with_context(|| "配置文件格式错误")?
        };
        config.resolve_env_placeholders()?;
        Ok(config)
    }

    pub fn resolve_env_placeholders(&mut self) -> Result<()> {
        for (_, provider) in &mut self.provider {
            if provider.options.api_key.starts_with("{env:") {
                let var_name = extract_env_var(&provider.options.api_key)?;
                provider.options.api_key = std::env::var(&var_name)
                    .with_context(|| format!("环境变量 {} 未设置", var_name))?;
            }
        }
        Ok(())
    }
}

fn extract_env_var(placeholder: &str) -> Result<String> {
    let start = placeholder
        .find("{env:")
        .ok_or_else(|| anyhow!("无效的环境变量占位符"))?
        + 5;
    let end = placeholder
        .find('}')
        .ok_or_else(|| anyhow!("占位符缺少闭合括号"))?;
    Ok(placeholder[start..end].to_string())
}

fn try_reload_config(
    res: Result<notify::Event, notify::Error>,
    last_reload: &Mutex<Instant>,
    config: &Arc<RwLock<Config>>,
) {
    let Ok(event) = res else { return };
    if !event.kind.is_modify() {
        return;
    }

    let mut last = last_reload.lock().unwrap();
    if last.elapsed() < Duration::from_millis(500) {
        return;
    }
    *last = Instant::now();
    drop(last);

    let Ok(new_config) = Config::load() else {
        log_info!("Warning: 配置热重载失败");
        return;
    };

    let Ok(mut cfg) = config.write() else {
        log_info!("Warning: 配置锁中毒，无法更新");
        return;
    };

    *cfg = new_config;
    log_info!("配置已热重载");
}

pub fn spawn_watcher(config: Arc<RwLock<Config>>) -> Result<impl notify::Watcher> {
    let config_dir = Config::config_dir();
    let last_reload = Arc::new(Mutex::new(Instant::now()));
    let last_reload_clone = Arc::clone(&last_reload);
    let config_clone = Arc::clone(&config);

    let mut watcher = notify::recommended_watcher(move |res| {
        try_reload_config(res, &last_reload_clone, &config_clone);
    })?;

    watcher.watch(&config_dir, notify::RecursiveMode::NonRecursive)?;
    Ok(watcher)
}
