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
