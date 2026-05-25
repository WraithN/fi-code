"""
测试项目管理模块
负责在 /tmp/fi_code_test 中创建和清理测试项目
"""
import os
import shutil
import subprocess
from pathlib import Path
from typing import List, Optional

from common import constants


class TestProjectManager:
    """测试项目管理器"""
    
    def __init__(self, project_dir: Path):
        self.project_dir = project_dir
        self.git_initialized = False
    
    def create_project(self):
        """创建测试项目"""
        print(f"{constants.COLOR_GREEN}创建测试项目: {self.project_dir}{constants.COLOR_RESET}")
        
        # 确保目录存在
        self.project_dir.mkdir(parents=True, exist_ok=True)
        
        # 创建一些基础文件
        self._create_basic_files()
        
        # 初始化 git 仓库
        self._init_git()
        
        print(f"{constants.COLOR_GREEN}测试项目创建完成!{constants.COLOR_RESET}")
    
    def _create_basic_files(self):
        """创建基础文件"""
        
        # 创建 README.md
        readme = self.project_dir / "README.md"
        readme.write_text("""# Test Project

This is a test project for fi-code E2E tests.
""")
        
        # 创建 src 目录和文件
        src_dir = self.project_dir / "src"
        src_dir.mkdir(exist_ok=True)
        
        # 创建 main.py
        main_py = src_dir / "main.py"
        main_py.write_text(
            "#!/usr/bin/env python3\n"
            '"""Main module for test project"""\n'
            "def main():\n"
            '    """Main function"""\n'
            '    print("Hello, World!")\n'
            "    return 42\n"
            "\n"
            'if __name__ == "__main__":\n'
            "    main()\n"
        )

        # 创建 utils.py
        utils_py = src_dir / "utils.py"
        utils_py.write_text(
            "#!/usr/bin/env python3\n"
            '"""Utility functions"""\n'
            "def add(a: int, b: int) -> int:\n"
            '    """Add two numbers"""\n'
            "    return a + b\n"
            "\n"
            "def multiply(a: int, b: int) -> int:\n"
            '    """Multiply two numbers"""\n'
            "    return a * b\n"
        )

        # 创建 tests 目录
        tests_dir = self.project_dir / "tests"
        tests_dir.mkdir(exist_ok=True)

        # 创建测试文件
        test_file = tests_dir / "test_utils.py"
        test_file.write_text(
            "#!/usr/bin/env python3\n"
            '"""Test utils module"""\n'
            "from src.utils import add, multiply\n"
            "\n"
            "def test_add():\n"
            "    assert add(2, 3) == 5\n"
            "\n"
            "def test_multiply():\n"
            "    assert multiply(2, 3) == 6\n"
        )
    
    def _init_git(self):
        """初始化 git 仓库"""
        try:
            subprocess.run(["git", "init"], cwd=self.project_dir, check=True, capture_output=True)
            subprocess.run(["git", "config", "user.name", "Test User"], cwd=self.project_dir, check=True, capture_output=True)
            subprocess.run(["git", "config", "user.email", "test@example.com"], cwd=self.project_dir, check=True, capture_output=True)
            subprocess.run(["git", "add", "."], cwd=self.project_dir, check=True, capture_output=True)
            subprocess.run(["git", "commit", "-m", "Initial commit"], cwd=self.project_dir, check=True, capture_output=True)
            self.git_initialized = True
            print(f"{constants.COLOR_GREEN}Git 仓库初始化完成!{constants.COLOR_RESET}")
        except Exception as e:
            print(f"{constants.COLOR_YELLOW}Git 初始化失败: {e}{constants.COLOR_RESET}")
    
    def cleanup(self):
        """清理测试项目"""
        if self.project_dir.exists():
            print(f"{constants.COLOR_YELLOW}清理测试项目: {self.project_dir}{constants.COLOR_RESET}")
            try:
                shutil.rmtree(self.project_dir)
                print(f"{constants.COLOR_GREEN}测试项目已清理!{constants.COLOR_RESET}")
            except Exception as e:
                print(f"{constants.COLOR_RED}清理项目时出错: {e}{constants.COLOR_RESET}")
    
    def create_file(self, path: str, content: str) -> Path:
        """创建文件"""
        file_path = self.project_dir / path
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_text(content)
        return file_path
    
    def read_file(self, path: str) -> Optional[str]:
        """读取文件内容"""
        file_path = self.project_dir / path
        if file_path.exists():
            return file_path.read_text()
        return None
    
    def file_exists(self, path: str) -> bool:
        """检查文件是否存在"""
        file_path = self.project_dir / path
        return file_path.exists()
    
    def get_file_list(self, pattern: str = "*") -> List[str]:
        """获取文件列表"""
        files = list(self.project_dir.glob(pattern))
        return [str(f.relative_to(self.project_dir)) for f in files]
    
    def run_command(self, cmd: List[str]) -> tuple[int, str, str]:
        """在项目目录中运行命令"""
        try:
            result = subprocess.run(
                cmd,
                cwd=self.project_dir,
                capture_output=True,
                text=True
            )
            return (result.returncode, result.stdout, result.stderr)
        except Exception as e:
            return (-1, "", str(e))
