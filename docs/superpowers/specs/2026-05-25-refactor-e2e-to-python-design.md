# E2E 测试重构设计文档：Rust → Python

> 将 `tests/e2e-tui/` 中的 Rust E2E 测试全部迁移为 Python，统一测试语言栈，并整合现有的 `tests/e2e-web/python/` Web E2E 测试。

---

## 1. 概述

### 1.1 背景

当前 fi-code 项目的 E2E 测试存在语言分裂问题：
- **TUI E2E**：使用 Rust 编写（`tests/e2e-tui/`），测试 CLI/TUI 二进制的基础功能和流程
- **Web E2E**：使用 Python + Playwright 编写（`tests/e2e-web/python/`），测试 Web 前端交互

这种分裂导致：
- 维护成本高（需要同时熟悉 Rust 测试和 Python 测试两种技术栈）
- 测试基础设施重复（两套 server 管理、fixtures、常量定义）
- 新增测试人员学习曲线陡峭

### 1.2 目标

1. **统一语言栈**：所有 E2E 测试使用 Python + pytest
2. **统一目录结构**：`tests/e2e/{common,cli,tui,web}/`
3. **用例粒化**：每个测试用例一个独立文件，文件名与用例名一一对应
4. **双模式触发**：支持 `pytest tests/e2e/` 和 `cargo test --test e2e_all`
5. **零功能损失**：迁移后的测试覆盖范围 ≥ 原有 Rust 测试

### 1.3 范围

| 模块 | 动作 | 说明 |
|------|------|------|
| `tests/e2e-tui/*.rs` | 删除 | 全部 Rust E2E 代码 |
| `tests/e2e-web/python/` | 迁移 | 整体迁移到 `tests/e2e/web/` |
| `tests/e2e-common/` | 重建 | 改为 Python 公共包 |
| `tests/Cargo.toml` | 修改 | 移除 Rust E2E targets，添加桥接 target |
| 新增 `tests/e2e/cli/` | 创建 | CLI 基础功能测试 |
| 新增 `tests/e2e/tui/` | 创建 | TUI 交互和流程测试 |
| 新增 `tests/e2e/common/` | 创建 | 公共 fixtures 和工具 |

---

## 2. 目录结构

```
tests/
├── Cargo.toml                 # 移除 Rust E2E targets，添加 e2e_all 桥接
├── bdd/                       # BDD 测试（不变）
│   ├── features/
│   └── steps/
├── bdd_test.rs               # BDD 入口（不变）
├── lib.rs                     # 测试库（不变）
└── e2e/                       # 【新增/重构】统一的 E2E 测试目录
    ├── conftest.py            # pytest 全局 fixtures 和配置
    ├── pytest.ini             # pytest 运行配置
    ├── run_e2e.py             # Cargo 桥接脚本入口
    ├── requirements.txt       # Python 依赖
    ├── README.md              # E2E 测试使用说明
    │
    ├── common/                # 公共工具包
    │   ├── __init__.py
    │   ├── constants.py       # 常量定义（从 e2e-web/python/constants.py 迁移扩展）
    │   ├── fixtures.py        # 共享 pytest fixtures
    │   ├── server.py          # 服务器管理（fi-code server 启动/停止/健康检查）
    │   ├── project.py         # 测试项目管理（创建/清理测试项目）
    │   ├── mock_ai.py         # Mock AI 服务器
    │   ├── mock_langfuse.py   # Mock Langfuse OTLP 服务器
    │   ├── vite_server.py     # Vite dev server 管理
    │   ├── pexpect_utils.py   # TUI pexpect 辅助函数
    │   └── subprocess_utils.py # CLI subprocess 辅助函数
    │
    ├── cli/                   # CLI E2E 测试
    │   ├── __init__.py
    │   ├── test_cli_help.py           # test_cli_help_flag_shows_usage
    │   ├── test_cli_version.py        # test_cli_version_flag_shows_version
    │   ├── test_cli_models.py         # test_cli_models_flag_lists_providers
    │   ├── test_cli_single_command.py # test_cli_single_command_mode
    │   ├── test_cli_session.py        # test_cli_session_flag_lists_sessions
    │   ├── test_cli_server.py         # test_cli_server_subcommand_starts_server
    │   └── test_cli_web_flag.py       # test_cli_web_flag_in_help
    │
    ├── tui/                   # TUI E2E 测试
    │   ├── __init__.py
    │   ├── test_tui_help.py              # test_tui_help_flag_shows_usage
    │   ├── test_tui_version.py           # test_tui_version_flag_shows_version
    │   ├── test_tui_backend_server.py    # test_tui_starts_backend_server
    │   ├── test_tui_simple_greeting.py   # test_simple_greeting_flow
    │   ├── test_tui_code_writing.py      # test_code_writing_flow
    │   ├── test_tui_task_splitting.py    # test_task_splitting_flow
    │   ├── test_tui_sse_lifecycle.py     # test_sse_stream_lifecycle
    │   └── test_tui_chat_session.py      # test_chat_with_existing_session
    │
    └── web/                   # Web E2E 测试（从 e2e-web/python 迁移）
        ├── __init__.py
        ├── test_web_simple_demo.py        # 原 test_web_01_simple_demo.py
        ├── test_web_tool_tests.py         # 原 test_web_02_tool_tests.py
        ├── test_web_basic_functionality.py # 原 test_web_basic_functionality.py
        ├── test_web_performance.py        # 原 test_web_performance.py
        ├── test_web_single_tools.py       # 原 test_web_single_tools.py
        ├── test_web_special_features.py   # 原 test_web_special_features.py
        ├── test_web_workflows.py          # 原 test_web_workflows.py
        ├── test_web_real_api_connection.py # 原 test_real_api_connection.py
        ├── test_web_e2e_real.py           # 原 test_web_e2e_real.py
        ├── test_web_observability.py      # 原 test_web_observability.py
        └── test_web_real_model.py         # 原 test_web_real_model.py
```

---

## 3. Common 公共包设计

### 3.1 职责划分

| 模块 | 职责 | 来源 |
|------|------|------|
| `constants.py` | 路径、端口、超时、颜色等常量 | 现有 `e2e-web/python/constants.py` 扩展 |
| `fixtures.py` | pytest fixtures：server、project、mock_ai、browser | 现有 `e2e-web/python/conftest.py` 拆分 |
| `server.py` | `FiCodeServerManager`：启动/停止/健康检查 fi-code server | 现有 `e2e-web/python/utils/server.py` |
| `project.py` | `TestProjectManager`：创建/清理测试项目 | 现有 `e2e-web/python/utils/project.py` |
| `mock_ai.py` | `MockAIServer`：模拟 AI 响应 | 现有 `e2e-web/python/utils/mock_ai.py` |
| `mock_langfuse.py` | `MockLangfuseServer`：模拟 Langfuse OTLP | 现有 `e2e-web/python/utils/mock_langfuse.py` |
| `vite_server.py` | `ViteDevServer`：前端 dev server | 现有 `e2e-web/python/utils/vite_server.py` |
| `pexpect_utils.py` | `TUIExpect`：pexpect 封装，发送按键、捕获界面输出 | **新增** |
| `subprocess_utils.py` | `run_binary()`、`assert_contains()` 等 | **新增** |

### 3.2 constants.py 关键常量

```python
# 二进制路径（从 cargo build 输出或环境变量获取）
PROJECT_ROOT = Path(__file__).parent.parent.parent.parent  # 到 fi-code 根目录
CLI_BIN = PROJECT_ROOT / "target" / "debug" / "fi-code-cli"
TUI_BIN = PROJECT_ROOT / "target" / "debug" / "fi-code-tui"
SERVER_BIN = PROJECT_ROOT / "target" / "debug" / "fi-code-cli"  # server 子命令

# 测试项目目录
TEST_PROJECT_DIR = Path("/tmp/fi_code_test")
TEST_TEMP_DIR = Path("/tmp/fi_code_test_temp")

# 默认端口（可被覆盖）
DEFAULT_SERVER_PORT = 14040
DEFAULT_FRONTEND_PORT = 15173
DEFAULT_MOCK_AI_PORT = 18080

# 超时
SERVER_START_TIMEOUT = 30  # 秒
PEXPECT_TIMEOUT = 10       # 秒
HTTP_TIMEOUT = 60          # 秒
```

### 3.3 subprocess_utils.py

```python
def run_binary(bin_path: Path, args: List[str], timeout: int = 10) -> subprocess.CompletedProcess:
    """运行 fi-code 二进制，返回输出"""
    ...

def assert_contains(output: str, expected: str) -> None:
    """断言输出包含预期字符串"""
    ...

def get_available_port() -> int:
    """获取随机可用端口"""
    ...
```

### 3.4 pexpect_utils.py

```python
class TUIExpect:
    """TUI pexpect 封装"""
    
    def __init__(self, bin_path: Path, args: List[str] = None):
        self.process = pexpect.spawn(str(bin_path), args or [])
    
    def send_key(self, key: str) -> None:
        """发送键盘事件"""
        ...
    
    def expect_text(self, text: str, timeout: int = 5) -> None:
        """期望界面出现指定文本"""
        ...
    
    def capture_screen(self) -> str:
        """捕获当前终端画面文本"""
        ...
    
    def close(self) -> None:
        """关闭 TUI 进程"""
        ...
    
    def __enter__(self):
        return self
    
    def __exit__(self, *args):
        self.close()
```

---

## 4. CLI 测试设计

### 4.1 测试策略

CLI 测试使用 Python `subprocess` 模块直接调用 `fi-code-cli` 二进制，捕获 stdout/stderr 进行断言。不涉及 HTTP API 或终端模拟。

### 4.2 用例映射

| 新文件 | 原 Rust 用例 | 验证内容 |
|--------|-------------|----------|
| `test_cli_help.py` | `test_cli_help_flag_shows_usage` | `--help` 输出包含 "fi-code" 和 "Usage:" |
| `test_cli_version.py` | `test_cli_version_flag_shows_version` | `--version` 输出包含版本号 "0.1.0" |
| `test_cli_models.py` | `test_cli_models_flag_lists_providers` | `--models` 输出包含 "Providers and Models" |
| `test_cli_single_command.py` | `test_cli_single_command_mode` | `-c "你好"` 输出包含输入内容 |
| `test_cli_session.py` | `test_cli_session_flag_lists_sessions` | `--session` 输出包含 "sessions" |
| `test_cli_server.py` | `test_cli_server_subcommand_starts_server` | `server --port 9999` 启动后 `/api/config` 返回 200 |
| `test_cli_web_flag.py` | `test_cli_web_flag_in_help` | `--help` 输出包含 `--web` 或 `-W` |

### 4.3 示例：`test_cli_help.py`

```python
"""
CLI Help 测试

验证 fi-code-cli --help 输出包含预期的使用说明。
"""
import pytest
from common.subprocess_utils import run_binary, assert_contains
from common.constants import CLI_BIN


def test_cli_help_flag_shows_usage():
    """test_cli_help_flag_shows_usage"""
    result = run_binary(CLI_BIN, ["--help"])
    assert result.returncode == 0
    output = result.stdout + result.stderr
    assert_contains(output, "fi-code")
    assert_contains(output, "Usage:")
```

### 4.4 示例：`test_cli_server.py`

```python
"""
CLI Server 子命令测试

验证 fi-code-cli server --port <port> 能成功启动 HTTP 服务。
"""
import urllib.request
import pytest
from common.subprocess_utils import get_available_port
from common.constants import CLI_BIN


def test_cli_server_subcommand_starts_server():
    """test_cli_server_subcommand_starts_server"""
    port = get_available_port()
    import subprocess
    proc = subprocess.Popen(
        [str(CLI_BIN), "server", "--port", str(port)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE
    )
    
    try:
        # 轮询等待服务就绪
        import time
        deadline = time.time() + 5
        while time.time() < deadline:
            try:
                resp = urllib.request.urlopen(
                    f"http://localhost:{port}/api/config", timeout=1
                )
                if resp.status == 200:
                    break
            except:
                time.sleep(0.5)
        else:
            pytest.fail("Server failed to start")
    finally:
        proc.terminate()
        proc.wait(timeout=5)
```

---

## 5. TUI 测试设计

### 5.1 测试策略

TUI 测试分为两种：

| 类型 | 工具 | 场景 |
|------|------|------|
| **基础功能** | `subprocess` | `--help`、`--version`、启动后端服务器（无需终端交互） |
| **交互流程** | `pexpect` + HTTP API | 问候语、代码书写、任务拆分、SSE 生命周期、会话对话 |

**为什么流程测试不用纯 pexpect？**
- TUI 界面使用 ratatui 渲染，纯文本捕获难以稳定断言复杂的界面状态
- 原有 Rust 测试已经验证了通过 HTTP API 可以完整驱动 TUI 后端逻辑
- HTTP API 测试更稳定、更易调试，pexpect 仅用于验证 TUI 界面启动和基础交互

### 5.2 用例映射

| 新文件 | 原 Rust 用例 | 测试方式 | 验证内容 |
|--------|-------------|----------|----------|
| `test_tui_help.py` | `test_tui_help_flag_shows_usage` | subprocess | `--help` 输出包含 "fi-code" 或 "Usage:" |
| `test_tui_version.py` | `test_tui_version_flag_shows_version` | subprocess | `--version` 输出包含 "0.1.0" |
| `test_tui_backend_server.py` | `test_tui_starts_backend_server` | subprocess | `FI_CODE_TEST_MODE=1 fi-code-tui --port <port>` 启动后 `/api/config` 返回 200 |
| `test_tui_simple_greeting.py` | `test_simple_greeting_flow` | HTTP API | 发送 "你好"，验证收到 Message 和 Done 事件 |
| `test_tui_code_writing.py` | `test_code_writing_flow` | HTTP API | 发送写代码请求，验证 write 工具调用和文件写入 |
| `test_tui_task_splitting.py` | `test_task_splitting_flow` | HTTP API | 发送复杂任务，验证 handle_task_plan 工具调用 |
| `test_tui_sse_lifecycle.py` | `test_sse_stream_lifecycle` | HTTP API | 验证 SSE 事件顺序：Message → Usage → Done |
| `test_tui_chat_session.py` | `test_chat_with_existing_session` | HTTP API | 创建会话 → 第一轮对话 → 第二轮对话，验证状态保持 |

### 5.3 示例：`test_tui_simple_greeting.py`

```python
"""
TUI 简单问候流程测试

验证通过 HTTP API 发送消息后，能收到预期的 SSE 事件流。
"""
import pytest
from common.server import FiCodeServerManager
from common.subprocess_utils import get_available_port
from common.constants import TUI_BIN, TEST_PROJECT_DIR


@pytest.fixture
def tui_server():
    """启动 TUI 后端服务器（FI_CODE_TEST_MODE=1）"""
    port = get_available_port()
    server = FiCodeServerManager(
        server_port=port,
        server_bin=TUI_BIN,
        project_dir=TEST_PROJECT_DIR,
        env={"FI_CODE_TEST_MODE": "1"}
    )
    server.start()
    yield server
    server.stop()


def test_simple_greeting_flow(tui_server):
    """test_simple_greeting_flow"""
    import urllib.request
    import json
    
    url = f"http://localhost:{tui_server.port}/chat"
    req = urllib.request.Request(
        url,
        data=json.dumps({"session_id": None, "message": "你好，你是谁"}).encode(),
        headers={"Content-Type": "application/json"},
        method="POST"
    )
    
    with urllib.request.urlopen(req, timeout=60) as resp:
        assert resp.status == 200
        # 读取 SSE 流并解析事件
        events = parse_sse_events(resp)
        
        message_events = [e for e in events if e["type"] == "Message"]
        assert len(message_events) > 0
        
        all_text = "".join(e.get("content", "") for e in message_events)
        assert "FiCode" in all_text or "编程" in all_text
        
        assert any(e["type"] == "Done" for e in events)
        assert not any(e["type"] == "Error" for e in events)
```

### 5.4 示例：`test_tui_help.py`

```python
"""
TUI Help 测试
"""
from common.subprocess_utils import run_binary, assert_contains
from common.constants import TUI_BIN


def test_tui_help_flag_shows_usage():
    """test_tui_help_flag_shows_usage"""
    result = run_binary(TUI_BIN, ["--help"])
    assert result.returncode == 0
    output = result.stdout + result.stderr
    assert "fi-code" in output or "Usage:" in output
```

---

## 6. Web 测试迁移设计

### 6.1 迁移策略

**文件级迁移**（不修改测试逻辑，仅调整路径和导入）：

| 原文件 | 新文件 | 动作 |
|--------|--------|------|
| `e2e-web/python/conftest.py` | `e2e/conftest.py` | 迁移并扩展 |
| `e2e-web/python/constants.py` | `e2e/common/constants.py` | 迁移 |
| `e2e-web/python/utils/*.py` | `e2e/common/*.py` | 迁移 |
| `e2e-web/python/test_02_web/test_web_01_simple_demo.py` | `e2e/web/test_web_simple_demo.py` | 迁移 |
| `e2e-web/python/test_02_web/test_web_02_tool_tests.py` | `e2e/web/test_web_tool_tests.py` | 迁移 |
| `e2e-web/python/test_02_web/test_web_basic_functionality.py` | `e2e/web/test_web_basic_functionality.py` | 迁移 |
| `e2e-web/python/test_02_web/test_web_performance.py` | `e2e/web/test_web_performance.py` | 迁移 |
| `e2e-web/python/test_02_web/test_web_single_tools.py` | `e2e/web/test_web_single_tools.py` | 迁移 |
| `e2e-web/python/test_02_web/test_web_special_features.py` | `e2e/web/test_web_special_features.py` | 迁移 |
| `e2e-web/python/test_02_web/test_web_workflows.py` | `e2e/web/test_web_workflows.py` | 迁移 |
| `e2e-web/python/test_real_api_connection.py` | `e2e/web/test_web_real_api_connection.py` | 迁移 |
| `e2e-web/python/test_web_e2e_real.py` | `e2e/web/test_web_e2e_real.py` | 迁移 |
| `e2e-web/python/test_web_observability.py` | `e2e/web/test_web_observability.py` | 迁移 |
| `e2e-web/python/test_web_real_model.py` | `e2e/web/test_web_real_model.py` | 迁移 |

### 6.2 导入调整

所有 Web 测试文件的导入需要调整：

```python
# 原导入
import constants
from utils.server import FiCodeServerManager

# 新导入
from common import constants
from common.server import FiCodeServerManager
```

### 6.3 conftest.py 调整

将原 `e2e-web/python/conftest.py` 迁移到 `tests/e2e/conftest.py`，并做以下调整：
1. 更新 `sys.path` 指向 `tests/e2e`
2. 更新导入路径
3. 添加 CLI/TUI 相关的 fixtures（如 `cli_bin`、`tui_bin`）

---

## 7. Cargo 集成设计

### 7.1 目标

运行 `cargo test --test e2e_all` 时，自动执行 `pytest tests/e2e/`。

### 7.2 实现方式

在 `tests/Cargo.toml` 中：

1. **移除**以下 Rust E2E test targets：
   - `e2e_tui_cli` (`e2e-tui/cli_e2e.rs`)
   - `e2e_tui_basic` (`e2e-tui/tui_e2e.rs`)
   - `e2e_tui_flow` (`e2e-tui/tui_flow_e2e.rs`)

2. **添加**新的桥接 target：
   ```toml
   [[test]]
   name = "e2e_all"
   path = "e2e/run_e2e.rs"
   harness = false
   ```

3. `tests/e2e/run_e2e.rs` 内容：
   ```rust
   use std::process::Command;
   
   fn main() {
       let status = Command::new("python3")
           .args(["-m", "pytest", "tests/e2e", "-v"])
           .status()
           .expect("Failed to run pytest");
       
       std::process::exit(if status.success() { 0 } else { 1 });
   }
   ```

### 7.3 依赖管理

`tests/e2e/requirements.txt`：

```
pytest>=8.0
pytest-asyncio>=0.23
pytest-timeout>=2.3
pexpect>=4.9
requests>=2.31
playwright>=1.40
psutil>=5.9
python-dotenv>=1.0
aiohttp>=3.9
```

---

## 8. 删除和清理计划

### 8.1 删除的文件/目录

| 路径 | 原因 |
|------|------|
| `tests/e2e-tui/cli_e2e.rs` | 迁移为 Python CLI 测试 |
| `tests/e2e-tui/tui_e2e.rs` | 迁移为 Python TUI 测试 |
| `tests/e2e-tui/tui_flow_e2e.rs` | 迁移为 Python TUI 测试 |
| `tests/e2e-common/README.md` | 重建为 Python 公共包 |
| `tests/e2e-web/python/` | 整体迁移到 `tests/e2e/web/` |

### 8.2 修改的文件

| 路径 | 修改内容 |
|------|----------|
| `tests/Cargo.toml` | 移除 3 个 E2E test targets，添加 `e2e_all` 桥接 target |
| `tests/e2e/conftest.py` | 新建，整合 fixtures |
| `tests/e2e/pytest.ini` | 新建，pytest 配置 |

---

## 9. 测试运行方式

### 9.1 运行全部 E2E 测试

```bash
# 方式 1：直接 pytest
pytest tests/e2e -v

# 方式 2：通过 Cargo
cargo test --test e2e_all
```

### 9.2 运行指定模块

```bash
# 仅 CLI
pytest tests/e2e/cli -v

# 仅 TUI
pytest tests/e2e/tui -v

# 仅 Web
pytest tests/e2e/web -v
```

### 9.3 运行单个用例

```bash
# 用例名与文件名一一对应，可直接指定文件
pytest tests/e2e/cli/test_cli_help.py -v
pytest tests/e2e/tui/test_tui_simple_greeting.py -v
pytest tests/e2e/web/test_web_basic_functionality.py -v
```

---

## 10. 风险与注意事项

### 10.1 风险

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| pexpect 在 CI 环境不稳定 | 中 | TUI 测试偶发失败 | TUI 核心流程使用 HTTP API，pexpect 仅用于简单验证 |
| 端口冲突 | 低 | 测试失败 | 使用 `get_available_port()` 动态分配端口 |
| 二进制未编译 | 高 | 测试无法运行 | 在 fixture 中检查二进制存在性，给出明确错误提示 |
| Web 测试迁移遗漏 import | 中 | ImportError | 迁移后统一运行 `pytest --collect-only` 检查 |

### 10.2 注意事项

1. **环境依赖**：运行 E2E 测试前需先执行 `cargo build`，确保二进制已生成
2. **Python 版本**：要求 Python 3.10+
3. **Playwright 安装**：首次运行 Web 测试需执行 `playwright install chromium`
4. **TUI 测试模式**：TUI 流程测试需要设置 `FI_CODE_TEST_MODE=1` 环境变量，跳过前端界面仅启动后端
5. **工作目录**：CLI/TUI 测试使用临时目录作为 workspace，避免污染用户真实项目

---

## 11. 验收标准

- [ ] `tests/e2e-tui/` 目录下无 Rust 文件
- [ ] `tests/e2e/` 目录结构符合本设计
- [ ] 所有原 Rust E2E 用例均有对应的 Python 实现
- [ ] 现有 Web 测试完整迁移到 `tests/e2e/web/`，且能正常运行
- [ ] `pytest tests/e2e` 全部通过
- [ ] `cargo test --test e2e_all` 全部通过
- [ ] `cargo test`（不含 E2E）不受影响，BDD 测试正常
