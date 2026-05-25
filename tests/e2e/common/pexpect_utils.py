"""
TUI pexpect 辅助函数
用于与 ratatui 终端界面交互
"""
import os
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
