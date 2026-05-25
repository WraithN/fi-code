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
