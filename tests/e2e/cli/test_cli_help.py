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
