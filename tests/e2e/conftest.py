"""
fi-code E2E 测试全局 fixtures
整合 CLI / TUI / Web 三种模式的共享资源
"""
import os
import sys
import asyncio
import time
from pathlib import Path
from typing import AsyncGenerator

# 添加 e2e 目录到路径，使 common 包可导入
sys.path.insert(0, str(Path(__file__).parent))

# 从 common 包导入 fixtures
from common.fixtures import *  # noqa: F401,F403

import pytest
from playwright.async_api import async_playwright, Page, Browser, BrowserContext

from common import constants
from common.server import FiCodeServerManager
from common.mock_ai import MockAIServer
from common.vite_server import ViteDevServer


@pytest.fixture(scope="function")
async def fi_code_server() -> AsyncGenerator[FiCodeServerManager, None]:
    """
    fi-code 服务器 fixture (function 级别)

    启动真实的 fi-code 服务器，使用随机端口避免冲突
    """
    server = FiCodeServerManager(
        server_port=constants.SERVER_PORT,
        server_bin=constants.SERVER_BIN,
        project_dir=constants.TEST_PROJECT_DIR
    )

    # 启动服务器
    await server.start()

    yield server

    # 停止服务器
    await server.stop()


@pytest.fixture(scope="function")
async def mock_ai_server() -> AsyncGenerator[MockAIServer, None]:
    """
    Mock AI 服务器 fixture (function 级别)

    提供模拟的 AI 响应，避免调用真实 API
    可以通过环境变量 USE_MOCK_AI=false 关闭
    """
    if not constants.USE_MOCK_AI:
        print(f"{constants.COLOR_YELLOW}⚠️  USE_MOCK_AI=false - 不启动 Mock AI 服务器{constants.COLOR_RESET}")
        yield None
        return

    print(f"{constants.COLOR_GREEN}✅ 启动 Mock AI 服务器{constants.COLOR_RESET}")
    mock_server = MockAIServer(port=constants.MOCK_SERVER_PORT)

    await mock_server.start()

    yield mock_server

    await mock_server.stop()


@pytest.fixture(scope="session")
def use_mock_ai() -> bool:
    """
    是否使用 Mock AI 的开关 fixture
    """
    return constants.USE_MOCK_AI


@pytest.fixture(scope="function")
async def browser() -> AsyncGenerator[Browser, None]:
    """
    Playwright 浏览器 fixture (function 级别)

    启动浏览器，所有测试共享同一个浏览器实例
    """
    async with async_playwright() as p:
        # 使用 Chromium 浏览器
        browser = await p.chromium.launch(
            headless=False,  # 可以设为 True 进行无头测试
            args=["--no-sandbox"] if os.getenv("CI") else []
        )
        yield browser
        await browser.close()


@pytest.fixture(scope="function")
async def context(browser: Browser) -> AsyncGenerator[BrowserContext, None]:
    """
    Playwright 浏览器上下文 fixture (function 级别)

    每个测试都有独立的上下文，实现隔离
    """
    context = await browser.new_context(
        viewport={"width": 1920, "height": 1080}
    )

    yield context

    await context.close()


@pytest.fixture(scope="function")
async def page(
    context: BrowserContext,
    fi_code_server: FiCodeServerManager
) -> AsyncGenerator[Page, None]:
    """
    Playwright 页面 fixture (function 级别)

    每个测试的页面，自动导航到前端地址
    """
    page = await context.new_page()

    # 设置页面超时
    page.set_default_timeout(constants.PAGE_LOAD_TIMEOUT)

    # 导航到前端
    # 这里我们需要先启动 Vite dev server，或者先构建前端文件
    # 暂时注释掉，后面会完善
    # await page.goto(f"http://localhost:{constants.FRONTEND_PORT}")

    yield page

    # 记录页面状态用于调试
    try:
        await page.screenshot(path=constants.TEST_TEMP_DIR / f"screenshot_{int(time.time())}.png")
    except:
        pass


@pytest.fixture(scope="function")
async def page_with_frontend(
    page: Page,
    fi_code_server: FiCodeServerManager
) -> AsyncGenerator[Page, None]:
    """
    完整的前端页面 fixture

    在启动前端服务后返回页面
    """
    # TODO: 实现 Vite 服务器启动和前端访问
    yield page


@pytest.fixture
def server_url(fi_code_server: FiCodeServerManager) -> str:
    """获取服务器 URL"""
    return f"http://localhost:{fi_code_server.port}"


@pytest.fixture
def frontend_url() -> str:
    """获取前端 URL"""
    return f"http://localhost:{constants.FRONTEND_PORT}"


@pytest.fixture(scope="function")
async def vite_server() -> AsyncGenerator[ViteDevServer, None]:
    """
    Vite 前端服务器 fixture (function 级别)

    启动 Vite dev server
    """
    frontend_dir = constants.PROJECT_ROOT / "frontend"
    server = ViteDevServer(
        port=constants.FRONTEND_PORT,
        frontend_dir=frontend_dir
    )

    try:
        await server.start()
        yield server
    except Exception as e:
        print(f"{constants.COLOR_RED}Vite 服务器启动失败: {e}{constants.COLOR_RESET}")
        print(f"{constants.COLOR_YELLOW}将使用简化测试模式...{constants.COLOR_RESET}")
        yield None
    finally:
        await server.stop()


@pytest.fixture(scope="function")
async def chat_page(
    page: Page,
    vite_server: ViteDevServer,
    fi_code_server: FiCodeServerManager
) -> AsyncGenerator[Page, None]:
    """
    完整的聊天页面 fixture

    会导航到聊天界面
    """
    if vite_server and vite_server.is_running:
        try:
            await page.goto(f"http://localhost:{constants.FRONTEND_PORT}")
            # 等待页面加载完成
            await asyncio.sleep(2)
        except Exception as e:
            print(f"{constants.COLOR_YELLOW}导航到前端失败: {e}{constants.COLOR_RESET}")

    yield page
