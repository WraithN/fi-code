"""
CLI Server 子命令测试
验证 fi-code-cli server --port <port> 能成功启动 HTTP 服务。
对应原 Rust 用例: test_cli_server_subcommand_starts_server
"""
import time
import urllib.request
import pytest
import psutil
import subprocess
from common.subprocess_utils import get_available_port
from common.constants import CLI_BIN, COLOR_GREEN, COLOR_RED, COLOR_RESET


@pytest.mark.cli
@pytest.mark.functional
def test_cli_server_subcommand_starts_server():
    """test_cli_server_subcommand_starts_server"""
    port = get_available_port()
    proc = subprocess.Popen(
        [str(CLI_BIN), "server", "--port", str(port)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE
    )

    try:
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
