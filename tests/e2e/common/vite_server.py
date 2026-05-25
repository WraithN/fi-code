"""
Vite 前端服务器管理模块
负责启动、停止 Vite dev server
"""
import asyncio
import subprocess
import os
import time
from pathlib import Path
from typing import Optional

from common import constants


class ViteDevServer:
    """Vite 开发服务器管理器"""
    
    def __init__(self, port: int, frontend_dir: Path):
        self.port = port
        self.frontend_dir = frontend_dir
        self.process: Optional[subprocess.Popen] = None
        self._is_running = False
    
    async def start(self):
        """启动 Vite 开发服务器"""
        print(f"{constants.COLOR_GREEN}启动 Vite 开发服务器 (端口: {self.port}){constants.COLOR_RESET}")
        
        # 检查前端目录是否存在
        if not self.frontend_dir.exists():
            raise FileNotFoundError(f"前端目录不存在: {self.frontend_dir}")
        
        # 检查 node_modules 是否存在
        node_modules = self.frontend_dir / "node_modules"
        if not node_modules.exists():
            print(f"{constants.COLOR_YELLOW}node_modules 不存在，尝试安装依赖...{constants.COLOR_RESET}")
            await self._install_deps()
        
        # 设置环境变量
        env = os.environ.copy()
        env["BROWSER"] = "none"
        
        # 启动 Vite 服务器
        cmd = ["npm", "run", "dev", "--", "--port", str(self.port)]
        
        print(f"{constants.COLOR_YELLOW}执行命令: {' '.join(cmd)}{constants.COLOR_RESET}")
        
        # 在后台启动进程
        self.process = subprocess.Popen(
            cmd,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=self.frontend_dir
        )
        
        # 等待服务器启动
        print(f"{constants.COLOR_YELLOW}等待 Vite 服务器启动...{constants.COLOR_RESET}")
        await asyncio.sleep(5)
        
        # 检查进程是否还在运行
        if self.process.poll() is not None:
            stdout, stderr = self.process.communicate()
            print(f"{constants.COLOR_RED}Vite 服务器启动失败!{constants.COLOR_RESET}")
            print(f"STDOUT:\n{stdout.decode() if stdout else ''}")
            print(f"STDERR:\n{stderr.decode() if stderr else ''}")
            raise RuntimeError("Vite 服务器启动失败")
        
        self._is_running = True
        print(f"{constants.COLOR_GREEN}Vite 开发服务器启动成功!{constants.COLOR_RESET}")
    
    async def _install_deps(self):
        """安装前端依赖"""
        cmd = ["npm", "install"]
        process = await asyncio.create_subprocess_exec(
            *cmd,
            cwd=self.frontend_dir,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE
        )
        
        stdout, stderr = await process.communicate()
        
        if process.returncode != 0:
            print(f"{constants.COLOR_YELLOW}依赖安装可能有问题，但继续尝试...{constants.COLOR_RESET}")
            print(f"STDOUT:\n{stdout.decode() if stdout else ''}")
            print(f"STDERR:\n{stderr.decode() if stderr else ''}")
    
    async def stop(self):
        """停止 Vite 开发服务器"""
        if not self.process or not self._is_running:
            return
        
        print(f"{constants.COLOR_YELLOW}停止 Vite 开发服务器...{constants.COLOR_RESET}")
        
        try:
            self.process.terminate()
            try:
                await asyncio.wait_for(asyncio.to_thread(self.process.wait), timeout=5)
            except asyncio.TimeoutError:
                print(f"{constants.COLOR_RED}强制终止 Vite 进程...{constants.COLOR_RESET}")
                self.process.kill()
                await asyncio.to_thread(self.process.wait, timeout=2)
        except Exception as e:
            print(f"{constants.COLOR_RED}停止 Vite 服务器时出错: {e}{constants.COLOR_RESET}")
        
        self._is_running = False
        print(f"{constants.COLOR_GREEN}Vite 开发服务器已停止{constants.COLOR_RESET}")
    
    @property
    def is_running(self) -> bool:
        """检查服务器是否在运行"""
        if not self.process:
            return False
        
        return self.process.poll() is None
    
    def get_logs(self) -> tuple[str, str]:
        """获取服务器日志"""
        if not self.process:
            return "", ""
        
        try:
            stdout = self.process.stdout.read() if self.process.stdout else b""
            stderr = self.process.stderr.read() if self.process.stderr else b""
            return stdout.decode(), stderr.decode()
        except:
            return "", ""
