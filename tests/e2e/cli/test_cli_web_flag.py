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
