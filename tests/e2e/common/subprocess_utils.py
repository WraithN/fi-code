"""
Subprocess 辅助函数
用于 CLI/TUI 基础功能测试
"""
import json
import os
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
    """
    assert expected in output, f"Expected output to contain '{expected}', but got:\n{output}"


def get_available_port() -> int:
    """
    获取随机可用端口
    """
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def parse_sse_events(response) -> list:
    """
    从 HTTP 响应中解析 SSE 事件流

    Args:
        response: urllib.request.urlopen 返回的响应对象

    Returns:
        事件列表，每个事件为 dict
    """
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
