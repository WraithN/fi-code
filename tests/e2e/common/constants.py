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
