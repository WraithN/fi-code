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
