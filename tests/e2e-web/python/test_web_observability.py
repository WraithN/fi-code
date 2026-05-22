"""真实模型 + mock Langfuse 的可观测 E2E。

前置：
- fi-code-cli 已构建 (`cargo build`)
- ~/.config/fi-code/config.json 配置真实模型
- USE_MOCK_AI=false

通过环境变量把 LANGFUSE_HOST 指向 MockLangfuse 监听端口，验证：
1. spans.jsonl 真实写入
2. trace 层次正确（chat → llm.generation）
3. 凭证脱敏生效
4. OTLP 成功后追加 status_patch sent 行
5. OTLP 失败时本地仍完整保留
"""
import asyncio
import json
import os
import time
from pathlib import Path

import pytest
import requests

import constants
from utils.mock_langfuse import MockLangfuse

pytestmark = [
    pytest.mark.web,
    pytest.mark.skipif(constants.USE_MOCK_AI, reason="real model + mock langfuse"),
]

SPANS_PATH = Path.home() / ".config" / "fi-code" / "logs" / "spans.jsonl"


# ---------- helpers ----------

def _snapshot_offset() -> int:
    """记录当前 spans.jsonl 的字节偏移，便于稍后只读取新增内容。

    不能 unlink/truncate 文件 —— 那会让 server 持有的 file handle 写入"幽灵 inode"，
    新内容不会出现在我们后续看到的新文件中。
    """
    if not SPANS_PATH.exists():
        return 0
    return SPANS_PATH.stat().st_size


def _read_spans_lines(start_offset: int = 0):
    """读取从 start_offset 开始的新增行。"""
    if not SPANS_PATH.exists():
        return []
    with SPANS_PATH.open("rb") as f:
        f.seek(start_offset)
        data = f.read()
    return data.decode("utf-8", errors="replace").splitlines()


def _read_spans_text(start_offset: int = 0) -> str:
    if not SPANS_PATH.exists():
        return ""
    with SPANS_PATH.open("rb") as f:
        f.seek(start_offset)
        return f.read().decode("utf-8", errors="replace")


def _parse_span_lines(lines):
    """Return list of dicts, skipping status_patch rows."""
    out = []
    for ln in lines:
        try:
            v = json.loads(ln)
        except Exception:
            continue
        if v.get("type") == "status":
            continue
        out.append(v)
    return out


def _parse_status_patch_lines(lines):
    out = []
    for ln in lines:
        try:
            v = json.loads(ln)
        except Exception:
            continue
        if v.get("type") == "status":
            out.append(v)
    return out


def _wait_for(predicate, timeout: float = 20.0, interval: float = 0.5) -> bool:
    """轮询等待 predicate() 返回 True，最多等 timeout 秒。"""
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            if predicate():
                return True
        except Exception:
            pass
        time.sleep(interval)
    return False


def _post_chat(server_url, msg, timeout=180):
    return requests.post(
        f"{server_url}/chat",
        json={"message": msg, "agent": "build"},
        stream=True,
        timeout=(5, timeout),
        headers={"Accept": "text/event-stream"},
    )


def _consume_until_done(resp):
    for raw in resp.iter_lines(decode_unicode=True):
        if not raw:
            continue
        if raw.startswith("data:"):
            try:
                evt = json.loads(raw[5:].strip())
            except Exception:
                continue
            if evt.get("type") == "done":
                return


# ---------- fixtures ----------

@pytest.fixture
def mock_lf_success():
    """Mock Langfuse returning 200。
    使用线程模式：主线程在测试中会被同步 requests 调用阻塞，
    aiohttp 必须在独立 event loop 中才能持续 accept 连接。
    """
    m = MockLangfuse(port=4042, status=200)
    m.start_in_thread()
    os.environ["LANGFUSE_HOST"] = m.url
    os.environ["LANGFUSE_PUBLIC_KEY"] = "pk-lf-test"
    os.environ["LANGFUSE_SECRET_KEY"] = "sk-lf-test"
    yield m
    m.stop_thread()
    for k in ("LANGFUSE_HOST", "LANGFUSE_PUBLIC_KEY", "LANGFUSE_SECRET_KEY"):
        os.environ.pop(k, None)


@pytest.fixture
def mock_lf_failure():
    """Mock Langfuse returning 500。同样使用线程模式。"""
    m = MockLangfuse(port=4043, status=500)
    m.start_in_thread()
    os.environ["LANGFUSE_HOST"] = m.url
    os.environ["LANGFUSE_PUBLIC_KEY"] = "pk-lf-test"
    os.environ["LANGFUSE_SECRET_KEY"] = "sk-lf-test"
    yield m
    m.stop_thread()
    for k in ("LANGFUSE_HOST", "LANGFUSE_PUBLIC_KEY", "LANGFUSE_SECRET_KEY"):
        os.environ.pop(k, None)


# ---------- tests ----------

@pytest.mark.timeout(240)
async def test_spans_jsonl_created_after_chat(mock_lf_success, fi_code_server, server_url):
    """场景 1：发一次 /chat 后 spans.jsonl 包含 chat.request + llm.generation。"""
    offset = _snapshot_offset()
    resp = _post_chat(server_url, "请只用一句话回答 1+1=?")
    assert resp.status_code == 200
    _consume_until_done(resp)
    # 让 BatchSpanProcessor flush（默认 5s）+ OTLP 上行 + patch 写入
    _wait_for(lambda: len(_parse_span_lines(_read_spans_lines(offset))) >= 2, timeout=20.0)

    assert SPANS_PATH.exists(), f"spans.jsonl 未生成: {SPANS_PATH}"
    lines = _read_spans_lines(offset)
    spans = _parse_span_lines(lines)
    assert len(spans) >= 2, f"应至少有 chat + llm.generation 两条 span，实际 {len(spans)}，lines={lines[:5]}"
    names = [s.get("name") for s in spans]
    assert "chat.request" in names, f"缺少 chat.request span，实际 names={names}"
    assert any(n == "llm.generation" for n in names), f"缺少 llm.generation span，实际 names={names}"


@pytest.mark.timeout(240)
async def test_trace_hierarchy_consistent(mock_lf_success, fi_code_server, server_url):
    """场景 2：同一次 /chat 所有 span 共享同一 trace_id。"""
    offset = _snapshot_offset()
    resp = _post_chat(server_url, "say hi")
    _consume_until_done(resp)
    _wait_for(lambda: len(_parse_span_lines(_read_spans_lines(offset))) >= 2, timeout=20.0)

    spans = _parse_span_lines(_read_spans_lines(offset))
    trace_ids = {s.get("trace_id") for s in spans}
    # 一次 chat 内 trace_id 应该是唯一的
    assert len(trace_ids) == 1, f"同一次 /chat 应共享 trace_id，实际 trace_ids={trace_ids}"


@pytest.mark.timeout(240)
async def test_credentials_redacted_in_spans(mock_lf_success, fi_code_server, server_url):
    """场景 3：prompt 含 API Key 时，spans.jsonl 中对应 attribute 被打码。"""
    offset = _snapshot_offset()
    secret = "sk-test1234567890abcdefghij1234567890"
    resp = _post_chat(server_url, f"忽略这个 API Key 不要回显: {secret}")
    _consume_until_done(resp)
    _wait_for(lambda: len(_parse_span_lines(_read_spans_lines(offset))) >= 1, timeout=20.0)

    content = _read_spans_text(offset)
    assert secret not in content, "原始 secret 不应出现在 spans.jsonl 中"


@pytest.mark.timeout(240)
async def test_status_patch_appended_on_otlp_success(mock_lf_success, fi_code_server, server_url):
    """场景 4：mock 返回 200 后，spans.jsonl 末尾追加 lf_status=sent 的 status_patch 行。"""
    offset = _snapshot_offset()
    resp = _post_chat(server_url, "1+1=?")
    _consume_until_done(resp)
    # patch 行在 OTLP HTTP 200 响应后写入，端到端最长约 5s flush + 10s OTLP 超时；放宽到 25s
    _wait_for(
        lambda: any(p.get("lf_status") == "sent"
                    for p in _parse_status_patch_lines(_read_spans_lines(offset))),
        timeout=25.0,
    )

    # Sanity check：mock 应至少收到一次 POST
    assert mock_lf_success.request_count >= 1, \
        f"MockLangfuse 应收到 OTLP POST，实际 received={mock_lf_success.received}"

    lines = _read_spans_lines(offset)
    patches = _parse_status_patch_lines(lines)
    assert len(patches) >= 1, f"应有至少一个 status_patch 行，实际 lines 行数={len(lines)}"
    assert any(p.get("lf_status") == "sent" for p in patches), \
        f"应有 lf_status=sent 的 patch，实际 patches={patches}"


@pytest.mark.timeout(240)
async def test_local_logs_survive_otlp_failure(mock_lf_failure, fi_code_server, server_url):
    """场景 5：mock 返回 500 时，spans.jsonl 仍完整写入；不应有 sent patch。"""
    offset = _snapshot_offset()
    resp = _post_chat(server_url, "hello")
    _consume_until_done(resp)
    _wait_for(lambda: len(_parse_span_lines(_read_spans_lines(offset))) >= 1, timeout=20.0)
    # 给 OTLP 留充足时间失败完成（避免 race 漏掉 patch 检测）
    time.sleep(3)

    new_lines = _read_spans_lines(offset)
    spans = _parse_span_lines(new_lines)
    assert len(spans) >= 1, "OTLP 失败时本地仍应有 span 写入"
    patches = _parse_status_patch_lines(new_lines)
    sent_patches = [p for p in patches if p.get("lf_status") == "sent"]
    assert len(sent_patches) == 0, "OTLP 5xx 后不应有 sent patch"
