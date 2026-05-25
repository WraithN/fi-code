"""
Mock AI 服务器模块
提供模拟的 AI 响应，避免调用真实 API
"""
import asyncio
import json
import random
from typing import Dict, Any, List, Optional, Callable
from aiohttp import web

from common import constants


class MockAIServer:
    """Mock AI 服务器"""
    
    def __init__(self, port: int):
        self.port = port
        self.app = web.Application()
        self.runner: Optional[web.AppRunner] = None
        self.site: Optional[web.TCPSite] = None
        
        # 预设响应
        self.response_handlers: Dict[str, Callable] = {}
        self._setup_default_handlers()
        
        # 注册路由
        self._setup_routes()
    
    def _setup_default_handlers(self):
        """设置默认响应处理器"""
        
        def default_chat_response(prompt: str) -> Dict[str, Any]:
            """默认聊天响应"""
            
            # 检测工具调用意图
            if "run" in prompt.lower() and ("bash" in prompt.lower() or "command" in prompt.lower()):
                return self._create_tool_use_response("bash", {"command": "echo 'Hello from mock!'"})
            
            if "read" in prompt.lower() and "file" in prompt.lower():
                return self._create_tool_use_response("read", {"path": "/tmp/test.txt"})
            
            if "write" in prompt.lower() and "file" in prompt.lower():
                return self._create_tool_use_response("write", {"path": "/tmp/test.txt", "content": "Hello, world!"})
            
            if "task" in prompt.lower() or "split" in prompt.lower():
                return self._create_tool_use_response("create_task_plan", {"goal": prompt})
            
            if "question" in prompt.lower():
                return self._create_tool_use_response("ask_for_question", {"question": "How can I help you?"})
            
            # 默认文本响应
            return {
                "type": "text",
                "content": "This is a mock AI response. I understand you asked something!"
            }
        
        self.response_handlers["default"] = default_chat_response
    
    def _create_tool_use_response(self, tool_name: str, args: Dict[str, Any]) -> Dict[str, Any]:
        """创建工具调用响应"""
        return {
            "type": "tool_use",
            "tool_name": tool_name,
            "tool_arguments": args
        }
    
    def _setup_routes(self):
        """设置 HTTP 路由"""
        
        async def handle_chat(request):
            """处理聊天请求"""
            data = await request.json()
            prompt = data.get("prompt", "")
            
            # 调用响应处理器
            handler = self.response_handlers.get("default")
            response = handler(prompt)
            
            return web.json_response(response)
        
        async def handle_health(request):
            """健康检查"""
            return web.json_response({"status": "ok"})
        
        self.app.router.add_post("/api/chat", handle_chat)
        self.app.router.add_get("/health", handle_health)
    
    async def start(self):
        """启动 Mock AI 服务器"""
        print(f"{constants.COLOR_GREEN}启动 Mock AI 服务器 (端口: {self.port}){constants.COLOR_RESET}")
        
        self.runner = web.AppRunner(self.app)
        await self.runner.setup()
        self.site = web.TCPSite(self.runner, "localhost", self.port)
        await self.site.start()
        
        print(f"{constants.COLOR_GREEN}Mock AI 服务器已启动!{constants.COLOR_RESET}")
    
    async def stop(self):
        """停止 Mock AI 服务器"""
        print(f"{constants.COLOR_YELLOW}停止 Mock AI 服务器...{constants.COLOR_RESET}")
        
        if self.runner:
            await self.runner.cleanup()
        
        print(f"{constants.COLOR_GREEN}Mock AI 服务器已停止{constants.COLOR_RESET}")
    
    def register_response_handler(self, name: str, handler: Callable[[str], Dict[str, Any]]):
        """注册自定义响应处理器"""
        self.response_handlers[name] = handler
    
    def set_simple_text_response(self, text: str):
        """设置简单的文本响应"""
        def handler(prompt: str):
            return {"type": "text", "content": text}
        self.response_handlers["default"] = handler
    
    def set_tool_sequence_response(self, tool_sequence: List[Dict[str, Any]]):
        """设置工具序列响应"""
        current_step = 0
        
        def handler(prompt: str):
            nonlocal current_step
            
            if current_step < len(tool_sequence):
                step = tool_sequence[current_step]
                current_step += 1
                return step
            
            # 序列结束后返回完成消息
            return {
                "type": "text",
                "content": "Task completed successfully!"
            }
        
        self.response_handlers["default"] = handler
