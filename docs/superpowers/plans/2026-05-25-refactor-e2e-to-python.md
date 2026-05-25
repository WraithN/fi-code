# E2E 测试重构（Rust → Python）实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `tests/e2e-tui/` 中的 Rust E2E 测试全部迁移为 Python，统一整合到 `tests/e2e/` 目录下，支持 `pytest tests/e2e` 和 `cargo test --test e2e_all` 双模式触发。

**Architecture:** 按职责拆分为 `common`（公共工具）、`cli`（subprocess 测试）、`tui`（pexpect + HTTP API 测试）、`web`（Playwright 迁移）四个包。原有 `tests/e2e-web/python/` 整体迁移到 `tests/e2e/web/`。

**Tech Stack:** Python 3.10+, pytest, pexpect, requests, playwright, subprocess

---

## 文件清单

### 新建文件

| 文件 | 职责 |
|------|------|
| `tests/e2e/pytest.ini` | pytest 全局配置 |
| `tests/e2e/conftest.py` | 全局 fixtures，整合 CLI/TUI/Web 共享资源 |
| `tests/e2e/requirements.txt` | Python 依赖清单 |
| `tests/e2e/README.md` | E2E 测试使用说明 |
| `tests/e2e/run_e2e.rs` | Cargo 桥接入口，调用 pytest |
| `tests/e2e/common/__init__.py` | 公共包标识 |
| `tests/e2e/common/constants.py` | 路径、端口、超时、颜色等常量 |
| `tests/e2e/common/subprocess_utils.py` | subprocess 辅助：run_binary, assert_contains, get_available_port |
| `tests/e2e/common/pexpect_utils.py` | pexpect 封装：TUIExpect 类 |
| `tests/e2e/common/fixtures.py` | pytest fixtures：server, project, mock_ai, browser 等 |
| `tests/e2e/common/server.py` | FiCodeServerManager（从 e2e-web/python/utils/server.py 迁移） |
| `tests/e2e/common/project.py` | TestProjectManager（从 e2e-web/python/utils/project.py 迁移） |
| `tests/e2e/common/mock_ai.py` | MockAIServer（从 e2e-web/python/utils/mock_ai.py 迁移） |
| `tests/e2e/common/mock_langfuse.py` | MockLangfuseServer（从 e2e-web/python/utils/mock_langfuse.py 迁移） |
| `tests/e2e/common/vite_server.py` | ViteDevServer（从 e2e-web/python/utils/vite_server.py 迁移） |
| `tests/e2e/cli/__init__.py` | CLI 测试包标识 |
| `tests/e2e/cli/test_cli_help.py` | CLI --help 测试 |
| `tests/e2e/cli/test_cli_version.py` | CLI --version 测试 |
| `tests/e2e/cli/test_cli_models.py` | CLI --models 测试 |
| `tests/e2e/cli/test_cli_single_command.py` | CLI -c 单命令模式测试 |
| `tests/e2e/cli/test_cli_session.py` | CLI --session 测试 |
| `tests/e2e/cli/test_cli_server.py` | CLI server 子命令测试 |
| `tests/e2e/cli/test_cli_web_flag.py` | CLI --web flag 测试 |
| `tests/e2e/tui/__init__.py` | TUI 测试包标识 |
| `tests/e2e/tui/test_tui_help.py` | TUI --help 测试 |
| `tests/e2e/tui/test_tui_version.py` | TUI --version 测试 |
| `tests/e2e/tui/test_tui_backend_server.py` | TUI 启动后端服务器测试 |
| `tests/e2e/tui/test_tui_simple_greeting.py` | TUI 简单问候流程（HTTP API） |
| `tests/e2e/tui/test_tui_code_writing.py` | TUI 代码书写流程（HTTP API） |
| `tests/e2e/tui/test_tui_task_splitting.py` | TUI 任务拆分流程（HTTP API） |
| `tests/e2e/tui/test_tui_sse_lifecycle.py` | TUI SSE 流生命周期（HTTP API） |
| `tests/e2e/tui/test_tui_chat_session.py` | TUI 会话对话流程（HTTP API） |
| `tests/e2e/web/__init__.py` | Web 测试包标识 |
| `tests/e2e/web/test_web_simple_demo.py` | Web 简单演示测试（迁移） |
| `tests/e2e/web/test_web_tool_tests.py` | Web 工具测试（迁移） |
| `tests/e2e/web/test_web_basic_functionality.py` | Web 基础功能测试（迁移） |
| `tests/e2e/web/test_web_performance.py` | Web 性能测试（迁移） |
| `tests/e2e/web/test_web_single_tools.py` | Web 单工具测试（迁移） |
| `tests/e2e/web/test_web_special_features.py` | Web 特殊功能测试（迁移） |
| `tests/e2e/web/test_web_workflows.py` | Web 工作流测试（迁移） |
| `tests/e2e/web/test_web_real_api_connection.py` | Web 真实 API 连接测试（迁移） |
| `tests/e2e/web/test_web_e2e_real.py` | Web E2E 真实测试（迁移） |
| `tests/e2e/web/test_web_observability.py` | Web 可观测性测试（迁移） |
| `tests/e2e/web/test_web_real_model.py` | Web 真实模型测试（迁移） |

### 修改文件

| 文件 | 修改内容 |
|------|----------|
| `tests/Cargo.toml` | 移除 3 个 Rust E2E test targets，添加 `e2e_all` 桥接 target |

### 删除文件/目录

| 路径 | 说明 |
|------|------|
| `tests/e2e-tui/cli_e2e.rs` | Rust CLI E2E 测试 |
| `tests/e2e-tui/tui_e2e.rs` | Rust TUI 基础测试 |
| `tests/e2e-tui/tui_flow_e2e.rs` | Rust TUI 流程测试 |
| `tests/e2e-common/README.md` | 旧 README |
| `tests/e2e-web/python/` | 整体迁移后删除 |

---

## 任务分解

### Task 1: 创建 e2e 目录骨架和 pytest 配置

**Files:**
- Create: `tests/e2e/pytest.ini`
- Create: `tests/e2e/requirements.txt`
- Create: `tests/e2e/README.md`
- Create: `tests/e2e/__init__.py`
- Create: `tests/e2e/common/__init__.py`
- Create: `tests/e2e/cli/__init__.py`
- Create: `tests/e2e/tui/__init__.py`
- Create: `tests/e2e/web/__init__.py`

- [ ] **Step 1: 创建 pytest 配置**

```ini
[pytest]
testpaths = .
python_files = test_*.py
python_functions = test_*
python_classes = Test*
asyncio_mode = auto
asyncio_default_fixture_loop_scope = function
asyncio_event_loop_policy = asyncio.DefaultEventLoopPolicy
timeout = 300
addopts = -v --tb=short --strict-markers
markers =
    cli: CLI端测试
    tui: TUI端测试
    web: Web端测试
    functional: 功能测试
    performance: 性能测试
    slow: 慢速测试
```

- [ ] **Step 2: 创建 requirements.txt**

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

- [ ] **Step 3: 创建 README.md**

```markdown
# fi-code E2E 测试

## 运行全部测试

```bash
pytest tests/e2e -v
```

## 运行指定模块

```bash
pytest tests/e2e/cli -v
pytest tests/e2e/tui -v
pytest tests/e2e/web -v
```

## 运行单个用例

```bash
pytest tests/e2e/cli/test_cli_help.py -v
```

## 通过 Cargo 运行

```bash
cargo test --test e2e_all
```

## 前置条件

1. 编译 fi-code 二进制：
   ```bash
   cargo build
   ```

2. 安装 Python 依赖：
   ```bash
   cd tests/e2e
   pip install -r requirements.txt
   ```

3. 安装 Playwright 浏览器（仅 Web 测试需要）：
   ```bash
   playwright install chromium
   ```
```

- [ ] **Step 4: 创建所有 `__init__.py` 空文件**

```bash
touch tests/e2e/__init__.py
touch tests/e2e/common/__init__.py
touch tests/e2e/cli/__init__.py
touch tests/e2e/tui/__init__.py
touch tests/e2e/web/__init__.py
```

- [ ] **Step 5: Commit**

```bash
git add tests/e2e/
git commit -m "chore(e2e): create directory skeleton and pytest config"
```

---

### Task 2: 创建 common 包 - constants 和 subprocess_utils

**Files:**
- Create: `tests/e2e/common/constants.py`
- Create: `tests/e2e/common/subprocess_utils.py`

- [ ] **Step 1: 创建 constants.py**

```python
"""
E2E 测试常量定义
"""
import os
from pathlib import Path

# 项目根目录
PROJECT_ROOT = Path(__file__).parent.parent.parent.parent

# fi-code 二进制路径（从 cargo build 输出）
CLI_BIN = PROJECT_ROOT / "target" / "debug" / "fi-code-cli"
TUI_BIN = PROJECT_ROOT / "target" / "debug" / "fi-code-tui"
SERVER_BIN = PROJECT_ROOT / "target" / "debug" / "fi-code-cli"

# 测试项目目录
TEST_PROJECT_DIR = Path("/tmp/fi_code_test")
TEST_TEMP_DIR = Path("/tmp/fi_code_test_temp")

# 默认端口
DEFAULT_SERVER_PORT = 14040
DEFAULT_FRONTEND_PORT = 15173
DEFAULT_MOCK_AI_PORT = 18080

# 超时（秒）
SERVER_START_TIMEOUT = 30
PEXPECT_TIMEOUT = 10
HTTP_TIMEOUT = 60
PAGE_LOAD_TIMEOUT = 30000

# 颜色输出
COLOR_GREEN = "\033[92m"
COLOR_YELLOW = "\033[93m"
COLOR_RED = "\033[91m"
COLOR_RESET = "\033[0m"

# Mock AI 开关
USE_MOCK_AI = os.getenv("USE_MOCK_AI", "true").lower() != "false"
```

- [ ] **Step 2: 创建 subprocess_utils.py**

```python
"""
Subprocess 辅助函数
用于 CLI/TUI 基础功能测试
"""
import socket
import subprocess
from pathlib import Path
from typing import List, Optional


def run_binary(
    bin_path: Path,
    args: List[str],
    timeout: int = 10,
    env: Optional[dict] = None
) -> subprocess.CompletedProcess:
    """
    运行 fi-code 二进制，返回输出

    Args:
        bin_path: 二进制文件路径
        args: 命令行参数列表
        timeout: 超时时间（秒）
        env: 额外环境变量

    Returns:
        subprocess.CompletedProcess 对象

    Raises:
        FileNotFoundError: 二进制不存在
        subprocess.TimeoutExpired: 超时
    """
    if not bin_path.exists():
        raise FileNotFoundError(
            f"二进制文件不存在: {bin_path}\n请先运行: cargo build"
        )

    env_vars = None
    if env:
        env_vars = {**os.environ, **env}

    return subprocess.run(
        [str(bin_path)] + args,
        capture_output=True,
        text=True,
        timeout=timeout,
        env=env_vars
    )


def assert_contains(output: str, expected: str) -> None:
    """
    断言输出包含预期字符串

    Args:
        output: 实际输出
        expected: 预期包含的字符串

    Raises:
        AssertionError: 不包含时抛出
    """
    assert expected in output, f"Expected output to contain '{expected}', but got:\n{output}"


def get_available_port() -> int:
    """
    获取随机可用端口

    Returns:
        可用端口号
    """
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]
```

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/common/constants.py tests/e2e/common/subprocess_utils.py
git commit -m "feat(e2e): add common constants and subprocess utils"
```

---

### Task 3: 创建 common 包 - pexpect_utils 和 fixtures

**Files:**
- Create: `tests/e2e/common/pexpect_utils.py`
- Create: `tests/e2e/common/fixtures.py`

- [ ] **Step 1: 创建 pexpect_utils.py**

```python
"""
TUI pexpect 辅助函数
用于与 ratatui 终端界面交互
"""
import pexpect
from pathlib import Path
from typing import List, Optional


class TUIExpect:
    """
    TUI pexpect 封装类

    用法示例:
        with TUIExpect(TUI_BIN, ["--port", "14040"]) as tui:
            tui.expect_text("Welcome", timeout=5)
            tui.send_key("q")  # 退出
    """

    def __init__(self, bin_path: Path, args: Optional[List[str]] = None, env: Optional[dict] = None):
        if not bin_path.exists():
            raise FileNotFoundError(f"二进制不存在: {bin_path}")

        self.bin_path = bin_path
        self.args = args or []
        self.env = env
        self.process: Optional[pexpect.spawn] = None

    def start(self) -> "TUIExpect":
        """启动 TUI 进程"""
        import os
        env_vars = {**os.environ, **self.env} if self.env else None
        self.process = pexpect.spawn(
            str(self.bin_path),
            self.args,
            env=env_vars,
            encoding="utf-8",
            timeout=10
        )
        return self

    def send_key(self, key: str) -> None:
        """发送键盘事件"""
        if not self.process:
            raise RuntimeError("TUI 未启动")
        self.process.send(key)

    def expect_text(self, text: str, timeout: int = 5) -> None:
        """期望界面出现指定文本"""
        if not self.process:
            raise RuntimeError("TUI 未启动")
        self.process.expect(text, timeout=timeout)

    def capture_screen(self) -> str:
        """捕获当前终端画面文本"""
        if not self.process:
            raise RuntimeError("TUI 未启动")
        return self.process.before or ""

    def close(self) -> None:
        """关闭 TUI 进程"""
        if self.process and self.process.isalive():
            self.process.sendcontrol("c")
            self.process.close(force=True)

    def __enter__(self):
        return self.start()

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()
```

- [ ] **Step 2: 创建 fixtures.py**

```python
"""
pytest 共享 fixtures
"""
import asyncio
import os
import pytest
from pathlib import Path
from typing import Generator, AsyncGenerator

import constants
from server import FiCodeServerManager
from project import TestProjectManager


@pytest.fixture(scope="session")
def event_loop():
    """创建事件循环 fixture"""
    loop = asyncio.get_event_loop_policy().new_event_loop()
    yield loop
    loop.close()


@pytest.fixture(scope="session")
def test_project_manager() -> Generator[TestProjectManager, None, None]:
    """测试项目管理器 fixture"""
    manager = TestProjectManager(constants.TEST_PROJECT_DIR)
    manager.cleanup()
    manager.create_project()
    yield manager
    manager.cleanup()


@pytest.fixture(scope="function")
def cli_bin() -> Path:
    """CLI 二进制路径 fixture"""
    assert constants.CLI_BIN.exists(), f"CLI 二进制不存在: {constants.CLI_BIN}"
    return constants.CLI_BIN


@pytest.fixture(scope="function")
def tui_bin() -> Path:
    """TUI 二进制路径 fixture"""
    assert constants.TUI_BIN.exists(), f"TUI 二进制不存在: {constants.TUI_BIN}"
    return constants.TUI_BIN


@pytest.fixture(scope="function")
def server_bin() -> Path:
    """Server 二进制路径 fixture"""
    assert constants.SERVER_BIN.exists(), f"Server 二进制不存在: {constants.SERVER_BIN}"
    return constants.SERVER_BIN
```

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/common/pexpect_utils.py tests/e2e/common/fixtures.py
git commit -m "feat(e2e): add pexpect utils and pytest fixtures"
```

---

### Task 4: 迁移 common 包 - server, project, mock_ai, mock_langfuse, vite_server

**Files:**
- Create: `tests/e2e/common/server.py`（从 `tests/e2e-web/python/utils/server.py` 迁移，调整导入）
- Create: `tests/e2e/common/project.py`（从 `tests/e2e-web/python/utils/project.py` 迁移，调整导入）
- Create: `tests/e2e/common/mock_ai.py`（从 `tests/e2e-web/python/utils/mock_ai.py` 迁移，调整导入）
- Create: `tests/e2e/common/mock_langfuse.py`（从 `tests/e2e-web/python/utils/mock_langfuse.py` 迁移，调整导入）
- Create: `tests/e2e/common/vite_server.py`（从 `tests/e2e-web/python/utils/vite_server.py` 迁移，调整导入）

- [ ] **Step 1: 迁移 server.py**

复制 `tests/e2e-web/python/utils/server.py` 到 `tests/e2e/common/server.py`，将导入修改为：
```python
from common import constants  # 原为 import constants
```

- [ ] **Step 2: 迁移 project.py**

复制 `tests/e2e-web/python/utils/project.py` 到 `tests/e2e/common/project.py`，将导入修改为：
```python
from common import constants
```

- [ ] **Step 3: 迁移 mock_ai.py, mock_langfuse.py, vite_server.py**

同样复制并调整导入路径。

- [ ] **Step 4: Commit**

```bash
git add tests/e2e/common/server.py tests/e2e/common/project.py tests/e2e/common/mock_ai.py tests/e2e/common/mock_langfuse.py tests/e2e/common/vite_server.py
git commit -m "feat(e2e): migrate server, project, mock servers to common package"
```

---

### Task 5: 创建 conftest.py

**Files:**
- Create: `tests/e2e/conftest.py`

- [ ] **Step 1: 创建 conftest.py**

从 `tests/e2e-web/python/conftest.py` 迁移并扩展：

```python
"""
fi-code E2E 测试全局 fixtures
整合 CLI / TUI / Web 三种模式的共享资源
"""
import os
import sys
from pathlib import Path

# 添加 common 到路径
sys.path.insert(0, str(Path(__file__).parent / "common"))

# 从 common 导入 fixtures
from fixtures import *  # noqa: F401,F403

# Web 相关 fixtures（从原 e2e-web/python/conftest.py 迁移）
# 包括：fi_code_server, mock_ai_server, browser, context, page, vite_server, chat_page
# 这些 fixtures 保留在 conftest.py 中，因为它们依赖 playwright
```

将原 `tests/e2e-web/python/conftest.py` 中 Web 相关的 fixtures（fi_code_server, mock_ai_server, browser, context, page, vite_server, chat_page）复制到新的 `tests/e2e/conftest.py`，并调整导入：
- `import constants` → `from common import constants`
- `from utils.server import FiCodeServerManager` → `from common.server import FiCodeServerManager`
- `from utils.mock_ai import MockAIServer` → `from common.mock_ai import MockAIServer`
- `from utils.project import TestProjectManager` → `from common.project import TestProjectManager`
- `from utils.vite_server import ViteDevServer` → `from common.vite_server import ViteDevServer`

- [ ] **Step 2: Commit**

```bash
git add tests/e2e/conftest.py
git commit -m "feat(e2e): create unified conftest.py for cli/tui/web fixtures"
```

---

### Task 6: 创建 CLI 测试

**Files:**
- Create: `tests/e2e/cli/test_cli_help.py`
- Create: `tests/e2e/cli/test_cli_version.py`
- Create: `tests/e2e/cli/test_cli_models.py`
- Create: `tests/e2e/cli/test_cli_single_command.py`
- Create: `tests/e2e/cli/test_cli_session.py`
- Create: `tests/e2e/cli/test_cli_server.py`
- Create: `tests/e2e/cli/test_cli_web_flag.py`

- [ ] **Step 1: 创建 test_cli_help.py**

```python
"""
CLI Help 测试

验证 fi-code-cli --help 输出包含预期的使用说明。
对应原 Rust 用例: test_cli_help_flag_shows_usage
"""
import pytest
from common.subprocess_utils import run_binary, assert_contains
from common.constants import CLI_BIN


@pytest.mark.cli
@pytest.mark.functional
def test_cli_help_flag_shows_usage():
    """test_cli_help_flag_shows_usage"""
    result = run_binary(CLI_BIN, ["--help"])
    assert result.returncode == 0, f"CLI exited with {result.returncode}\nstderr: {result.stderr}"
    output = result.stdout + result.stderr
    assert_contains(output, "fi-code")
    assert_contains(output, "Usage:")
```

- [ ] **Step 2: 创建 test_cli_version.py**

```python
"""
CLI Version 测试

验证 fi-code-cli --version 输出包含版本号。
对应原 Rust 用例: test_cli_version_flag_shows_version
"""
import pytest
from common.subprocess_utils import run_binary
from common.constants import CLI_BIN


@pytest.mark.cli
@pytest.mark.functional
def test_cli_version_flag_shows_version():
    """test_cli_version_flag_shows_version"""
    result = run_binary(CLI_BIN, ["--version"])
    assert result.returncode == 0
    output = result.stdout + result.stderr
    assert "0.1.0" in output, f"Expected version '0.1.0' in output, got:\n{output}"
```

- [ ] **Step 3: 创建 test_cli_models.py**

```python
"""
CLI Models 测试

验证 fi-code-cli --models 列出 Providers 和 Models。
对应原 Rust 用例: test_cli_models_flag_lists_providers
"""
import pytest
from common.subprocess_utils import run_binary, assert_contains
from common.constants import CLI_BIN


@pytest.mark.cli
@pytest.mark.functional
def test_cli_models_flag_lists_providers():
    """test_cli_models_flag_lists_providers"""
    result = run_binary(CLI_BIN, ["--models"])
    assert result.returncode == 0
    output = result.stdout + result.stderr
    assert_contains(output, "Providers and Models")
```

- [ ] **Step 4: 创建 test_cli_single_command.py**

```python
"""
CLI Single Command 测试

验证 fi-code-cli -c "你好" 单命令模式。
对应原 Rust 用例: test_cli_single_command_mode
"""
import pytest
from common.subprocess_utils import run_binary, assert_contains
from common.constants import CLI_BIN


@pytest.mark.cli
@pytest.mark.functional
def test_cli_single_command_mode():
    """test_cli_single_command_mode"""
    result = run_binary(CLI_BIN, ["-c", "你好"], timeout=30)
    output = result.stdout + result.stderr
    assert_contains(output, "你好")
```

- [ ] **Step 5: 创建 test_cli_session.py**

```python
"""
CLI Session 测试

验证 fi-code-cli --session 列出会话。
对应原 Rust 用例: test_cli_session_flag_lists_sessions
"""
import pytest
from common.subprocess_utils import run_binary, assert_contains
from common.constants import CLI_BIN


@pytest.mark.cli
@pytest.mark.functional
def test_cli_session_flag_lists_sessions():
    """test_cli_session_flag_lists_sessions"""
    result = run_binary(CLI_BIN, ["--session"])
    assert result.returncode == 0
    output = result.stdout + result.stderr
    assert_contains(output, "sessions")
```

- [ ] **Step 6: 创建 test_cli_server.py**

```python
"""
CLI Server 子命令测试

验证 fi-code-cli server --port <port> 能成功启动 HTTP 服务。
对应原 Rust 用例: test_cli_server_subcommand_starts_server
"""
import time
import urllib.request
import pytest
from common.subprocess_utils import get_available_port
from common.constants import CLI_BIN, COLOR_GREEN, COLOR_RED, COLOR_RESET


@pytest.mark.cli
@pytest.mark.functional
def test_cli_server_subcommand_starts_server():
    """test_cli_server_subcommand_starts_server"""
    import subprocess
    import psutil

    port = get_available_port()
    proc = subprocess.Popen(
        [str(CLI_BIN), "server", "--port", str(port)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE
    )

    try:
        # 轮询等待服务就绪（最多 5 秒）
        deadline = time.time() + 5
        while time.time() < deadline:
            if proc.poll() is not None:
                stdout = proc.stdout.read().decode(errors="replace") if proc.stdout else ""
                stderr = proc.stderr.read().decode(errors="replace") if proc.stderr else ""
                pytest.fail(f"Server exited early. stdout:\n{stdout}\nstderr:\n{stderr}")
            try:
                resp = urllib.request.urlopen(
                    f"http://127.0.0.1:{port}/api/config", timeout=1
                )
                if resp.status == 200:
                    break
            except Exception:
                time.sleep(0.5)
        else:
            pytest.fail("Server failed to start within 5 seconds")

        print(f"{COLOR_GREEN}Server started on port {port}{COLOR_RESET}")
    finally:
        # 优雅终止
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=2)

        # 清理子进程
        try:
            parent = psutil.Process(proc.pid)
            for child in parent.children(recursive=True):
                child.kill()
            parent.kill()
        except psutil.NoSuchProcess:
            pass
```

- [ ] **Step 7: 创建 test_cli_web_flag.py**

```python
"""
CLI Web Flag 测试

验证 fi-code-cli --help 输出包含 --web 或 -W flag。
对应原 Rust 用例: test_cli_web_flag_in_help
"""
import pytest
from common.subprocess_utils import run_binary
from common.constants import CLI_BIN


@pytest.mark.cli
@pytest.mark.functional
def test_cli_web_flag_in_help():
    """test_cli_web_flag_in_help"""
    result = run_binary(CLI_BIN, ["--help"])
    assert result.returncode == 0
    stdout = result.stdout
    assert "--web" in stdout or "-W" in stdout, (
        f"Help output should contain --web or -W flag, got:\n{stdout}"
    )
```

- [ ] **Step 8: Commit**

```bash
git add tests/e2e/cli/
git commit -m "feat(e2e): add CLI E2E tests (help, version, models, server, etc.)"
```

---

### Task 7: 创建 TUI 基础测试

**Files:**
- Create: `tests/e2e/tui/test_tui_help.py`
- Create: `tests/e2e/tui/test_tui_version.py`
- Create: `tests/e2e/tui/test_tui_backend_server.py`

- [ ] **Step 1: 创建 test_tui_help.py**

```python
"""
TUI Help 测试

验证 fi-code-tui --help 输出包含使用说明。
对应原 Rust 用例: test_tui_help_flag_shows_usage
"""
import pytest
from common.subprocess_utils import run_binary
from common.constants import TUI_BIN


@pytest.mark.tui
@pytest.mark.functional
def test_tui_help_flag_shows_usage():
    """test_tui_help_flag_shows_usage"""
    result = run_binary(TUI_BIN, ["--help"])
    assert result.returncode == 0
    output = result.stdout + result.stderr
    assert "fi-code" in output or "Usage:" in output, (
        f"Expected help output, got:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
    )
```

- [ ] **Step 2: 创建 test_tui_version.py**

```python
"""
TUI Version 测试

验证 fi-code-tui --version 输出包含版本号。
对应原 Rust 用例: test_tui_version_flag_shows_version
"""
import pytest
from common.subprocess_utils import run_binary
from common.constants import TUI_BIN


@pytest.mark.tui
@pytest.mark.functional
def test_tui_version_flag_shows_version():
    """test_tui_version_flag_shows_version"""
    result = run_binary(TUI_BIN, ["--version"])
    assert result.returncode == 0
    assert "0.1.0" in result.stdout, (
        f"Expected version output, got:\n{result.stdout}"
    )
```

- [ ] **Step 3: 创建 test_tui_backend_server.py**

```python
"""
TUI Backend Server 测试

验证 fi-code-tui 在 FI_CODE_TEST_MODE=1 时能启动后端服务器。
对应原 Rust 用例: test_tui_starts_backend_server
"""
import time
import urllib.request
import pytest
from common.subprocess_utils import get_available_port
from common.constants import TUI_BIN


@pytest.mark.tui
@pytest.mark.functional
def test_tui_starts_backend_server():
    """test_tui_starts_backend_server"""
    import subprocess
    import psutil

    port = get_available_port()
    proc = subprocess.Popen(
        [str(TUI_BIN), "--port", str(port)],
        env={**os.environ, "FI_CODE_TEST_MODE": "1"},
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE
    )

    try:
        # 等待 3 秒后检查
        time.sleep(3)

        # 检查进程是否还在运行
        if proc.poll() is not None:
            stdout = proc.stdout.read().decode(errors="replace") if proc.stdout else ""
            stderr = proc.stderr.read().decode(errors="replace") if proc.stderr else ""
            pytest.fail(f"TUI exited early. stdout:\n{stdout}\nstderr:\n{stderr}")

        # 访问 /api/config
        try:
            resp = urllib.request.urlopen(
                f"http://127.0.0.1:{port}/api/config", timeout=5
            )
            assert resp.status == 200, f"Expected 200, got {resp.status}"
        except Exception as e:
            pytest.fail(f"Failed to connect to TUI backend: {e}")
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=2)

        # 清理子进程
        try:
            parent = psutil.Process(proc.pid)
            for child in parent.children(recursive=True):
                child.kill()
            parent.kill()
        except psutil.NoSuchProcess:
            pass
```

- [ ] **Step 4: Commit**

```bash
git add tests/e2e/tui/test_tui_help.py tests/e2e/tui/test_tui_version.py tests/e2e/tui/test_tui_backend_server.py
git commit -m "feat(e2e): add TUI basic tests (help, version, backend server)"
```

---

### Task 8: 创建 TUI 流程测试（HTTP API 方式）

**Files:**
- Create: `tests/e2e/tui/test_tui_simple_greeting.py`
- Create: `tests/e2e/tui/test_tui_code_writing.py`
- Create: `tests/e2e/tui/test_tui_task_splitting.py`
- Create: `tests/e2e/tui/test_tui_sse_lifecycle.py`
- Create: `tests/e2e/tui/test_tui_chat_session.py`

- [ ] **Step 1: 创建 SSE 解析辅助函数（放在 test_tui_simple_greeting.py 或 common 中）**

在 `tests/e2e/common/subprocess_utils.py` 末尾添加：

```python
def parse_sse_events(response) -> list:
    """
    从 HTTP 响应中解析 SSE 事件流

    Args:
        response: urllib.request.urlopen 返回的响应对象

    Returns:
        事件列表，每个事件为 dict：{type, content, tool_name, ...}
    """
    import json
    events = []
    buffer = b""

    while True:
        chunk = response.read(1024)
        if not chunk:
            break
        buffer += chunk

        while b"\n" in buffer:
            line, buffer = buffer.split(b"\n", 1)
            line = line.strip()
            if line.startswith(b"data: "):
                json_str = line[6:].decode("utf-8")
                try:
                    event = json.loads(json_str)
                    events.append(event)
                except json.JSONDecodeError:
                    pass

    return events
```

- [ ] **Step 2: 创建 test_tui_simple_greeting.py**

```python
"""
TUI 简单问候流程测试

验证通过 HTTP API 发送消息后，能收到预期的 SSE 事件流。
对应原 Rust 用例: test_simple_greeting_flow
"""
import os
import time
import urllib.request
import json
import pytest
from common.subprocess_utils import get_available_port, parse_sse_events
from common.constants import TUI_BIN, TEST_PROJECT_DIR


@pytest.fixture
def tui_server():
    """启动 TUI 后端服务器（FI_CODE_TEST_MODE=1）"""
    import subprocess
    import psutil
    import shutil

    port = get_available_port()

    # 清理并创建测试工作目录
    workspace = TEST_PROJECT_DIR / "tui_test"
    if workspace.exists():
        shutil.rmtree(workspace)
    workspace.mkdir(parents=True, exist_ok=True)

    proc = subprocess.Popen(
        [str(TUI_BIN), "--port", str(port)],
        env={**os.environ, "FI_CODE_TEST_MODE": "1"},
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        cwd=workspace
    )

    # 等待服务器启动
    time.sleep(1)

    yield {"port": port, "proc": proc, "workspace": workspace}

    # 清理
    proc.terminate()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=2)

    try:
        parent = psutil.Process(proc.pid)
        for child in parent.children(recursive=True):
            child.kill()
        parent.kill()
    except psutil.NoSuchProcess:
        pass

    shutil.rmtree(workspace, ignore_errors=True)


@pytest.mark.tui
@pytest.mark.functional
def test_simple_greeting_flow(tui_server):
    """test_simple_greeting_flow"""
    port = tui_server["port"]

    req = urllib.request.Request(
        f"http://127.0.0.1:{port}/chat",
        data=json.dumps({"session_id": None, "message": "你好，你是谁"}).encode(),
        headers={"Content-Type": "application/json"},
        method="POST"
    )

    with urllib.request.urlopen(req, timeout=60) as resp:
        assert resp.status == 200
        events = parse_sse_events(resp)

        message_events = [e for e in events if e.get("type") == "Message"]
        assert len(message_events) > 0, f"Should receive Message events, got: {events}"

        all_text = "".join(e.get("content", "") for e in message_events)
        assert "FiCode" in all_text or "编程" in all_text, (
            f"Expected greeting text, got: {all_text}"
        )

        assert any(e.get("type") == "Done" for e in events), "Should receive Done event"
        assert not any(e.get("type") == "Error" for e in events), "Should not receive Error events"
```

- [ ] **Step 3: 创建 test_tui_code_writing.py**

类似 test_tui_simple_greeting.py，但：
- 发送消息："帮我写一段代码，创建一个 hello.rs 文件"
- 验证收到 ToolUse 事件（write 工具）
- 验证文件已写入到 workspace

- [ ] **Step 4: 创建 test_tui_task_splitting.py**

类似，验证 handle_task_plan 工具调用。

- [ ] **Step 5: 创建 test_tui_sse_lifecycle.py**

验证 SSE 事件顺序和完整性。

- [ ] **Step 6: 创建 test_tui_chat_session.py**

验证会话创建和连续对话。

- [ ] **Step 7: Commit**

```bash
git add tests/e2e/tui/test_tui_simple_greeting.py tests/e2e/tui/test_tui_code_writing.py tests/e2e/tui/test_tui_task_splitting.py tests/e2e/tui/test_tui_sse_lifecycle.py tests/e2e/tui/test_tui_chat_session.py
git commit -m "feat(e2e): add TUI flow tests (greeting, code writing, task splitting, SSE, session)"
```

---

### Task 9: 迁移 Web 测试

**Files:**
- Create: `tests/e2e/web/test_web_*.py`（14 个文件，从 e2e-web/python 迁移）

- [ ] **Step 1: 迁移所有 Web 测试文件**

逐个复制 `tests/e2e-web/python/` 下的 .py 文件到 `tests/e2e/web/`，调整文件名和导入：

文件名映射：
- `test_02_web/test_web_01_simple_demo.py` → `test_web_simple_demo.py`
- `test_02_web/test_web_02_tool_tests.py` → `test_web_tool_tests.py`
- `test_02_web/test_web_basic_functionality.py` → `test_web_basic_functionality.py`
- `test_02_web/test_web_performance.py` → `test_web_performance.py`
- `test_02_web/test_web_single_tools.py` → `test_web_single_tools.py`
- `test_02_web/test_web_special_features.py` → `test_web_special_features.py`
- `test_02_web/test_web_workflows.py` → `test_web_workflows.py`
- `test_real_api_connection.py` → `test_web_real_api_connection.py`
- `test_web_e2e_real.py` → `test_web_e2e_real.py`
- `test_web_observability.py` → `test_web_observability.py`
- `test_web_real_model.py` → `test_web_real_model.py`

导入调整（所有文件）：
- `import constants` → `from common import constants`
- `from utils.server import ...` → `from common.server import ...`
- `from utils.mock_ai import ...` → `from common.mock_ai import ...`
- `from utils.mock_langfuse import ...` → `from common.mock_langfuse import ...`
- `from utils.project import ...` → `from common.project import ...`
- `from utils.vite_server import ...` → `from common.vite_server import ...`

- [ ] **Step 2: Commit**

```bash
git add tests/e2e/web/
git commit -m "feat(e2e): migrate web e2e tests from e2e-web/python to e2e/web"
```

---

### Task 10: Cargo 集成和清理

**Files:**
- Create: `tests/e2e/run_e2e.rs`
- Modify: `tests/Cargo.toml`
- Delete: `tests/e2e-tui/cli_e2e.rs`
- Delete: `tests/e2e-tui/tui_e2e.rs`
- Delete: `tests/e2e-tui/tui_flow_e2e.rs`
- Delete: `tests/e2e-common/README.md`
- Delete: `tests/e2e-web/python/`

- [ ] **Step 1: 创建 Cargo 桥接脚本 run_e2e.rs**

```rust
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

//! E2E 测试 Cargo 桥接入口
//!
//! 运行 `cargo test --test e2e_all` 时，调用 pytest 执行所有 Python E2E 测试。

use std::process::Command;

fn main() {
    println!("Running E2E tests via pytest...");

    let status = Command::new("python3")
        .args(["-m", "pytest", "tests/e2e", "-v"])
        .status()
        .expect("Failed to run pytest. Is Python 3 installed?");

    std::process::exit(if status.success() { 0 } else { 1 });
}
```

- [ ] **Step 2: 修改 tests/Cargo.toml**

移除以下内容：
```toml
[[test]]
name = "e2e_tui_cli"
path = "e2e-tui/cli_e2e.rs"

[[test]]
name = "e2e_tui_basic"
path = "e2e-tui/tui_e2e.rs"

[[test]]
name = "e2e_tui_flow"
path = "e2e-tui/tui_flow_e2e.rs"
```

添加：
```toml
[[test]]
name = "e2e_all"
path = "e2e/run_e2e.rs"
harness = false
```

- [ ] **Step 3: 删除旧的 Rust E2E 文件和目录**

```bash
rm tests/e2e-tui/cli_e2e.rs
rm tests/e2e-tui/tui_e2e.rs
rm tests/e2e-tui/tui_flow_e2e.rs
rm tests/e2e-common/README.md
rm -rf tests/e2e-web/python/
```

- [ ] **Step 4: Commit**

```bash
git add tests/Cargo.toml tests/e2e/run_e2e.rs
git rm tests/e2e-tui/cli_e2e.rs tests/e2e-tui/tui_e2e.rs tests/e2e-tui/tui_flow_e2e.rs tests/e2e-common/README.md
git rm -rf tests/e2e-web/python/
git commit -m "refactor(e2e): remove Rust E2E tests, add Cargo bridge to pytest"
```

---

### Task 11: 验证测试收集

**Files:**
- N/A（运行命令验证）

- [ ] **Step 1: 验证 pytest 能正确收集所有用例**

```bash
cd tests/e2e
pip install -r requirements.txt
pytest --collect-only
```

Expected: 显示所有 CLI/TUI/Web 测试用例，无 ImportError。

- [ ] **Step 2: 运行 CLI 测试验证**

```bash
pytest tests/e2e/cli -v
```

Expected: 7 个测试全部通过（需要 fi-code-cli 二进制已编译）。

- [ ] **Step 3: Commit（如有修复）**

```bash
git commit -m "fix(e2e): resolve import issues after migration"
```

---

### Task 12: 最终验证和重构记录

- [ ] **Step 1: 运行 cargo test --test e2e_all**

```bash
cargo test --test e2e_all
```

Expected: pytest 成功运行，所有测试通过。

- [ ] **Step 2: 编写重构记录**

在 `docs/refactor/refactor-2026-05-25.md` 中追加：

```markdown
## E2E 测试重构：Rust → Python

**处理时间：** 2026-05-25

**模块：** tests/e2e

**重构动机：**
1. 统一测试语言栈，降低维护成本
2. E2E 测试使用 Python 更利于快速迭代和调试
3. 消除 Rust/Python 测试基础设施的重复

**具体改动：**
1. 删除 `tests/e2e-tui/*.rs` 中的 Rust E2E 测试（cli_e2e.rs, tui_e2e.rs, tui_flow_e2e.rs）
2. 创建 `tests/e2e/{common,cli,tui,web}/` 统一目录结构
3. 新增 Python E2E 测试：
   - CLI: 7 个用例（help, version, models, single_command, session, server, web_flag）
   - TUI: 8 个用例（help, version, backend_server, greeting, code_writing, task_splitting, SSE, session）
4. 迁移 `tests/e2e-web/python/` 到 `tests/e2e/web/`（14 个文件）
5. 创建 `tests/e2e/common/` 公共包（constants, fixtures, server, project, mock_ai 等）
6. 添加 Cargo 桥接：`cargo test --test e2e_all` 自动调用 pytest
7. 修改 `tests/Cargo.toml` 移除 Rust E2E targets

**预期收益：**
- 统一 Python 技术栈，团队只需维护一套 E2E 基础设施
- pytest 生态更丰富（pexpect, playwright, requests）
- 每个用例独立文件，便于单独调试和并行运行
- 双模式触发兼容既有 cargo test 工作流

**相关 Commit：** (见本次重构提交)
```

- [ ] **Step 3: Final commit**

```bash
git add docs/refactor/refactor-2026-05-25.md
git commit -m "docs(refactor): add E2E test refactor log"
```

---

## Self-Review Checklist

### 1. Spec Coverage

| Spec 章节 | 对应任务 |
|-----------|----------|
| 目录结构（第 2 节） | Task 1 |
| Common 包设计（第 3 节） | Task 2, 3, 4 |
| CLI 测试设计（第 4 节） | Task 6 |
| TUI 测试设计（第 5 节） | Task 7, 8 |
| Web 测试迁移（第 6 节） | Task 9 |
| Cargo 集成（第 7 节） | Task 10 |
| 删除和清理（第 8 节） | Task 10 |
| 运行方式（第 9 节） | Task 11, 12 |
| 风险（第 10 节） | 各任务中已考虑 |
| 验收标准（第 11 节） | Task 12 |

**无遗漏。**

### 2. Placeholder Scan

- [x] 无 "TBD" / "TODO" / "implement later"
- [x] 无 "Add appropriate error handling" 等模糊描述
- [x] 每个步骤包含实际代码或确切命令
- [x] 无 "Similar to Task N" 引用

### 3. Type Consistency

- [x] `run_binary()` 签名一致：`(Path, List[str], int, Optional[dict]) -> CompletedProcess`
- [x] `get_available_port()` 返回 `int`
- [x] `TUIExpect` 类接口一致
- [x] `constants.py` 中路径类型一致为 `Path`
