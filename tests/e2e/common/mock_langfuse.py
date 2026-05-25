"""Mock Langfuse OTLP endpoint for e2e tests.

Listens on a configurable port for POST /api/public/otel/v1/traces.
Records all incoming requests so tests can assert on header / body.
Default status 200 (success); can be configured to return 4xx/5xx for failure tests.

提供两种启动方式：
- async start()/stop()：在调用方事件循环内运行 aiohttp server。
- threaded start_in_thread()/stop_thread()：在独立线程 + 独立事件循环中运行，
  避免主线程被同步 I/O（如 requests）阻塞时 mock 收不到 TCP 连接。
"""
import asyncio
import threading
import time
from typing import List, Dict, Any, Optional

from aiohttp import web


class MockLangfuse:
    """Minimal mock of Langfuse OTLP ingestion endpoint."""

    def __init__(self, port: int = 4042, status: int = 200):
        self.port = port
        self.status = status
        self.received: List[Dict[str, Any]] = []
        self._runner: Optional[web.AppRunner] = None
        # 线程模式相关
        self._thread: Optional[threading.Thread] = None
        self._thread_loop: Optional[asyncio.AbstractEventLoop] = None
        self._thread_runner: Optional[web.AppRunner] = None

    async def _handle(self, request: web.Request) -> web.Response:
        body = await request.read()
        self.received.append({
            "headers": dict(request.headers),
            "body_bytes": len(body),
            "method": request.method,
            "path": request.path,
        })
        return web.Response(status=self.status)

    def _build_app(self) -> web.Application:
        app = web.Application()
        # Match both /api/public/otel/v1/traces (signal-specific) and /api/public/otel (generic)
        app.router.add_post("/api/public/otel/v1/traces", self._handle)
        app.router.add_post("/api/public/otel", self._handle)
        return app

    # ===== 异步模式（仅在调用方事件循环不被阻塞时使用） =====
    async def start(self) -> None:
        app = self._build_app()
        self._runner = web.AppRunner(app)
        await self._runner.setup()
        site = web.TCPSite(self._runner, "127.0.0.1", self.port)
        await site.start()

    async def stop(self) -> None:
        if self._runner is not None:
            await self._runner.cleanup()
            self._runner = None

    # ===== 线程模式（主线程可能被 sync requests 阻塞时使用） =====
    def start_in_thread(self, ready_timeout: float = 5.0) -> None:
        """在独立线程中跑事件循环，确保主线程被阻塞时 mock 仍能响应。"""
        ready_event = threading.Event()

        def _run():
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            self._thread_loop = loop

            async def _setup():
                app = self._build_app()
                self._thread_runner = web.AppRunner(app)
                await self._thread_runner.setup()
                site = web.TCPSite(self._thread_runner, "127.0.0.1", self.port)
                await site.start()
                ready_event.set()

            loop.run_until_complete(_setup())
            loop.run_forever()

        self._thread = threading.Thread(target=_run, daemon=True, name=f"MockLangfuse-{self.port}")
        self._thread.start()
        if not ready_event.wait(timeout=ready_timeout):
            raise RuntimeError(f"MockLangfuse failed to start within {ready_timeout}s")

    def stop_thread(self, timeout: float = 5.0) -> None:
        """关闭独立线程中的事件循环与 runner。"""
        if self._thread_loop is None:
            return

        async def _cleanup():
            if self._thread_runner is not None:
                await self._thread_runner.cleanup()
                self._thread_runner = None

        try:
            fut = asyncio.run_coroutine_threadsafe(_cleanup(), self._thread_loop)
            fut.result(timeout=timeout)
        except Exception:
            pass
        try:
            self._thread_loop.call_soon_threadsafe(self._thread_loop.stop)
        except Exception:
            pass
        if self._thread is not None:
            self._thread.join(timeout=timeout)
            self._thread = None
        self._thread_loop = None

    @property
    def url(self) -> str:
        return f"http://127.0.0.1:{self.port}"

    @property
    def request_count(self) -> int:
        return len(self.received)
