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
