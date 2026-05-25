"""
pytest 共享 fixtures
"""
import asyncio
import os
import pytest
from pathlib import Path
from typing import Generator

from common import constants
from common.server import FiCodeServerManager
from common.project import TestProjectManager


@pytest.fixture(scope="session")
def event_loop():
    """创建事件循环 fixture"""
    loop = asyncio.get_event_loop_policy().new_event_loop()
    yield loop
    loop.close()


@pytest.fixture(scope="session")
def test_project_manager() -> Generator[TestProjectManager, None, None]:
    """测试项目管理器 fixture"""
    manager = TestProjectManager(constants.TEST_PROJECT_DIR)
    manager.cleanup()
    manager.create_project()
    yield manager
    manager.cleanup()


@pytest.fixture(scope="function")
def cli_bin() -> Path:
    """CLI 二进制路径 fixture"""
    assert constants.CLI_BIN.exists(), f"CLI 二进制不存在: {constants.CLI_BIN}"
    return constants.CLI_BIN


@pytest.fixture(scope="function")
def tui_bin() -> Path:
    """TUI 二进制路径 fixture"""
    assert constants.TUI_BIN.exists(), f"TUI 二进制不存在: {constants.TUI_BIN}"
    return constants.TUI_BIN


@pytest.fixture(scope="function")
def server_bin() -> Path:
    """Server 二进制路径 fixture"""
    assert constants.SERVER_BIN.exists(), f"Server 二进制不存在: {constants.SERVER_BIN}"
    return constants.SERVER_BIN
