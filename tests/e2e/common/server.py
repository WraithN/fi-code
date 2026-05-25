"""
fi-code 服务器管理模块
负责启动、停止、监控 fi-code 服务器进程
"""
import asyncio
import subprocess
import time
import os
from pathlib import Path
from typing import Optional

import psutil
import urllib.request
import urllib.error

from common import constants


class FiCodeServerManager:
    """fi-code 服务器管理器"""
    
    def __init__(self, server_port: int, server_bin: Path, project_dir: Path):
        self.port = server_port
        self.server_bin = server_bin
        self.project_dir = project_dir
        self.process: Optional[subprocess.Popen] = None
        self._is_running = False
    
    async def start(self):
        """启动 fi-code 服务器"""
        print(f"{constants.COLOR_GREEN}启动 fi-code 服务器 (端口: {self.port}){constants.COLOR_RESET}")
        
        # 确保项目目录存在
        self.project_dir.mkdir(parents=True, exist_ok=True)
        
        # 检查服务器二进制是否存在
        if not self.server_bin.exists():
            raise FileNotFoundError(
                f"服务器二进制文件不存在: {self.server_bin}\n"
                f"请先运行 cargo build 构建服务器"
            )
        
        # 设置环境变量
        env = os.environ.copy()
        env["FI_CODE_TEST_MODE"] = "1"
        
        # 启动服务器（使用 fi-code-cli server --port）
        # -w 指定 workspace，server 子命令仅支持 --port
        cmd = [
            str(self.server_bin),
            "-w", str(self.project_dir),
            "server",
            "--port", str(self.port),
        ]
        
        print(f"{constants.COLOR_YELLOW}执行命令: {' '.join(cmd)}{constants.COLOR_RESET}")
        
        # 在后台启动进程
        self.process = subprocess.Popen(
            cmd,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=self.project_dir
        )
        
        # 等待服务器启动并完成 HTTP 健康检查（最多 30 秒）
        ready = await self._wait_http_ready(timeout_seconds=30)
        if not ready:
            stdout, stderr = self.process.communicate(timeout=2) if self.process.poll() is not None else (b"", b"")
            print(f"{constants.COLOR_RED}服务器健康检查失败!{constants.COLOR_RESET}")
            print(f"STDOUT:\n{stdout.decode(errors='replace') if stdout else ''}")
            print(f"STDERR:\n{stderr.decode(errors='replace') if stderr else ''}")
            raise RuntimeError("fi-code 服务器健康检查超时")

        self._is_running = True
        print(f"{constants.COLOR_GREEN}fi-code 服务器启动成功!{constants.COLOR_RESET}")

    async def _wait_http_ready(self, timeout_seconds: int = 30) -> bool:
        """通过轮询 /api/config 判断 HTTP 服务是否就绪"""
        url = f"http://127.0.0.1:{self.port}/api/config"
        deadline = time.time() + timeout_seconds
        while time.time() < deadline:
            # 子进程是否已意外退出
            if self.process and self.process.poll() is not None:
                return False
            try:
                with urllib.request.urlopen(url, timeout=2) as resp:
                    if 200 <= resp.status < 500:
                        return True
            except (urllib.error.URLError, ConnectionError, OSError):
                pass
            await asyncio.sleep(0.5)
        return False
    
    async def stop(self):
        """停止 fi-code 服务器"""
        if not self.process or not self._is_running:
            return
        
        print(f"{constants.COLOR_YELLOW}停止 fi-code 服务器...{constants.COLOR_RESET}")
        
        # 尝试优雅关闭
        try:
            self.process.terminate()
            try:
                await asyncio.wait_for(asyncio.to_thread(self.process.wait), timeout=5)
            except asyncio.TimeoutError:
                print(f"{constants.COLOR_RED}强制终止服务器进程...{constants.COLOR_RESET}")
                self.process.kill()
                await asyncio.to_thread(self.process.wait, timeout=2)
        except Exception as e:
            print(f"{constants.COLOR_RED}停止服务器时出错: {e}{constants.COLOR_RESET}")
        
        # 清理所有子进程
        try:
            parent = psutil.Process(self.process.pid)
            for child in parent.children(recursive=True):
                try:
                    child.kill()
                except:
                    pass
            parent.kill()
        except:
            pass
        
        self._is_running = False
        print(f"{constants.COLOR_GREEN}fi-code 服务器已停止{constants.COLOR_RESET}")
    
    @property
    def is_running(self) -> bool:
        """检查服务器是否在运行"""
        if not self.process:
            return False
        
        return self.process.poll() is None
    
    def get_server_logs(self) -> tuple[str, str]:
        """获取服务器日志"""
        if not self.process:
            return "", ""
        
        try:
            stdout = self.process.stdout.read() if self.process.stdout else b""
            stderr = self.process.stderr.read() if self.process.stderr else b""
            return stdout.decode(), stderr.decode()
        except:
            return "", ""
    
    async def wait_for_ready(self, timeout: int = 30) -> bool:
        """等待服务器准备就绪"""
        start_time = time.time()
        
        while time.time() - start_time < timeout:
            if self._is_running:
                # TODO: 实现健康检查逻辑
                await asyncio.sleep(1)
                return True
            
            await asyncio.sleep(0.5)
        
        return False
