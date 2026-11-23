"""Local hook and model fixtures. The model server accepts loopback requests only."""

import json
import sys
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


def record(*fields):
    message = " ".join(fields)
    with Path("model-events.txt").open("a", encoding="utf-8") as log:
        print(message, file=log)


def hook(project):
    event = json.load(sys.stdin)
    name = event["hook_event_name"]
    events = project / "hook-events.txt"
    with events.open("a", encoding="utf-8") as log:
        print(f"{name} {event['tool_name']} entered", file=log)
    if name == "PreToolUse":
        tool_input = event["tool_input"]
        tool_input["answers"] = {q["question"]: "Blue" for q in tool_input["questions"]}
        decision = {"permissionDecision": "allow", "updatedInput": tool_input}
        result = "allow Blue"
    else:
        release = project / "decision.txt"
        until = time.monotonic() + 45
        while not release.exists() and time.monotonic() < until:
            time.sleep(0.05)
        result = release.read_text(encoding="utf-8").strip() if release.exists() else "fallthrough"
        assert result in ("allow", "deny", "fallthrough")
        decision = {"decision": {"behavior": result, **(
            {"message": "fixture-hook-denied"} if result == "deny" else {})}}
    with events.open("a", encoding="utf-8") as log:
        print(f"{name} returned {result}", file=log)
    output = {} if result == "fallthrough" else {
        "hookSpecificOutput": {"hookEventName": name, **decision}
    }
    print(json.dumps(output), flush=True)


def anthropic(body):
    messages = [json.dumps(msg["content"], ensure_ascii=False) for msg in body["messages"]]
    last = next((msg for msg in reversed(messages) if any(marker in msg for marker in
        ("tool_result", "question-probe", "permission-probe", "probe-first", "interrupt-probe",
         "native-direct-probe"))),
        messages[-1])
    if "tool_result" in last:
        content = [{"type": "text", "text": "fixture-result-seen"}]
        reason = "end_turn"
        record("Claude tool result", "Blue" if "Blue" in last else
              "denied" if '"is_error": true' in last else
              "allowed" if "fixture-shell-ok" in last else "other")
    elif "question-probe" in last:
        content = [{
            "type": "tool_use", "id": "toolu_fixture", "name": "AskUserQuestion",
            "input": {"questions": [{
                "question": "Which colour?", "header": "Colour", "multiSelect": False,
                "options": [{"label": "Red", "description": "First option"},
                            {"label": "Blue", "description": "Second option"}],
            }]},
        }]
        reason = "tool_use"
    elif "permission-probe" in last:
        content = [{"type": "tool_use", "id": "toolu_fixture", "name": "Bash",
                    "input": {"command": "echo fixture-shell-ok", "description": "Print a fixture marker"}}]
        reason = "tool_use"
    else:
        content = [{"type": "text", "text": "fixture-text-seen"}]
        reason = "end_turn"
    record("Claude prompt", "multiline" if "probe-first\\nprobe-second\\nprobe-third" in last
           else "native-direct-probe" if "native-direct-probe" in last else "received")
    usage = {"input_tokens": 13, "output_tokens": 9, "cache_creation_input_tokens": 0,
             "cache_read_input_tokens": 0}
    message = {"id": "msg_fixture", "type": "message", "role": "assistant",
               "model": body["model"], "content": content, "stop_reason": reason,
               "stop_sequence": None, "usage": usage}
    if not body.get("stream"):
        return message
    events = [{"type": "message_start", "message": {**message, "content": [], "stop_reason": None}}]
    for index, block in enumerate(content):
        if block["type"] == "tool_use":
            start = {**block, "input": {}}
            delta = {"type": "input_json_delta", "partial_json": json.dumps(block["input"])}
        else:
            start = {**block, "text": ""}
            delta = {"type": "text_delta", "text": block["text"]}
        events.extend([
            {"type": "content_block_start", "index": index, "content_block": start},
            {"type": "content_block_delta", "index": index, "delta": delta},
            {"type": "content_block_stop", "index": index},
        ])
    events.extend([
        {"type": "message_delta", "delta": {"stop_reason": reason, "stop_sequence": None}, "usage": usage},
        {"type": "message_stop"},
    ])
    return events


def responses(body):
    last = body["input"][-1]
    text = json.dumps(last)
    if last["type"] in ("function_call_output", "custom_tool_call_output"):
        record("Codex tool result", "allowed" if "fixture-shell-ok" in text else "denied")
        reply = "fixture-result-seen"
    else:
        record("Codex prompt", "multiline" if "probe-first\\nprobe-second\\nprobe-third" in text
               else "native-direct-probe" if "native-direct-probe" in text else "received")
        reply = "fixture-text-seen"
    if "permission-probe" in text and last["type"] == "message":
        args = {"cmd": "echo fixture-shell-ok", "login": False, "sandbox_permissions": "require_escalated",
                "justification": "Print the Phase 0 fixture marker"}
        item = {"type": "function_call", "id": "fc_fixture", "call_id": "call_fixture",
                "name": "exec_command", "arguments": json.dumps(args), "status": "completed"}
    else:
        item = {"type": "message", "id": "msg_fixture", "role": "assistant", "status": "completed",
                "content": [{"type": "output_text", "text": reply, "annotations": []}]}
    response = {"id": "resp_fixture", "object": "response", "model": body["model"],
                "status": "completed", "output": [item], "usage": {
                    "input_tokens": 13, "input_tokens_details": {"cached_tokens": 0},
                    "output_tokens": 9, "output_tokens_details": {"reasoning_tokens": 0},
                    "total_tokens": 22}}
    return [
        {"type": "response.created", "response": {**response, "status": "in_progress", "output": []}},
        {"type": "response.output_item.added", "output_index": 0, "item": item},
        {"type": "response.output_item.done", "output_index": 0, "item": item},
        {"type": "response.completed", "response": response},
    ]


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        self.reply({"data": []})

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        if self.path.startswith("/v1/messages/count_tokens"):
            self.reply({"input_tokens": 13})
        elif self.path.startswith("/v1/messages"):
            if any("interrupt-probe" in json.dumps(msg["content"]) for msg in body["messages"][-3:]):
                record("Claude interrupt request started")
                time.sleep(10)
            self.reply(anthropic(body))
        elif self.path.startswith("/v1/responses"):
            if "interrupt-probe" in json.dumps(body["input"][-1]):
                record("Codex interrupt request started")
                time.sleep(10)
            self.reply(responses(body))
        else:
            self.reply({})

    def reply(self, result):
        if isinstance(result, list):
            kind = "text/event-stream"
            payload = "".join(f"event: {r['type']}\ndata: {json.dumps(r)}\n\n" for r in result)
        else:
            kind, payload = "application/json", json.dumps(result)
        data = payload.encode()
        self.send_response(200)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        try:
            self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError):
            pass


if __name__ == "__main__":
    if sys.argv[1] == "hook":
        hook(Path(sys.argv[2]))
    else:
        with ThreadingHTTPServer(("127.0.0.1", 0), Handler) as server:
            print(server.server_port, flush=True)
            server.serve_forever()
