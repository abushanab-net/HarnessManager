//! Anthropic Messages <-> OpenAI Chat Completions, for requests, responses and streams.


use serde_json::{json, Value};

fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

/// Anthropic Messages request -> OpenAI Chat Completions request for `model`.
pub fn anthropic_to_openai(req: &Value, model: &str) -> Value {
    let mut msgs = vec![];
    let system = text_of(&req["system"]);
    if !system.is_empty() {
        msgs.push(json!({"role": "system", "content": system}));
    }
    for m in req["messages"].as_array().into_iter().flatten() {
        let role = m["role"].as_str().unwrap_or("user");
        let content = &m["content"];
        if let Value::String(s) = content {
            msgs.push(json!({"role": role, "content": s}));
            continue;
        }
        let blocks = content.as_array().cloned().unwrap_or_default();
        if role == "assistant" {
            let text: Vec<&str> = blocks.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect();
            let calls: Vec<Value> = blocks.iter().filter(|b| b["type"] == "tool_use").map(|b| json!({
                "id": b["id"], "type": "function",
                "function": {"name": b["name"], "arguments": b["input"].to_string()}
            })).collect();
            // a turn with only thinking (a reasoning model cut off by max_tokens) has nothing OpenAI accepts
            if text.is_empty() && calls.is_empty() { continue; }
            let mut out = json!({"role": "assistant", "content": if text.is_empty() { Value::Null } else { json!(text.join("\n")) }});
            if !calls.is_empty() { out["tool_calls"] = json!(calls); }
            msgs.push(out);
            continue;
        }
        // user: tool results become tool messages, the rest one user message
        for b in blocks.iter().filter(|b| b["type"] == "tool_result") {
            let mut c = text_of(&b["content"]);
            if b["is_error"] == true { c = format!("Error: {c}"); }
            msgs.push(json!({"role": "tool", "tool_call_id": b["tool_use_id"], "content": c}));
        }
        // tool messages cannot carry images: those in tool results follow in the user message
        let inner: Vec<Value> = blocks.iter().filter(|b| b["type"] == "tool_result")
            .flat_map(|b| b["content"].as_array().cloned().unwrap_or_default()).filter(|c| c["type"] == "image").collect();
        let parts: Vec<Value> = blocks.iter().chain(&inner).filter_map(|b| match b["type"].as_str() {
            Some("text") => Some(json!({"type": "text", "text": b["text"]})),
            Some("image") if b["source"]["type"] == "base64" => Some(json!({"type": "image_url", "image_url": {
                "url": format!("data:{};base64,{}", b["source"]["media_type"].as_str().unwrap_or("image/png"), b["source"]["data"].as_str().unwrap_or(""))}})),
            Some("image") => b["source"]["url"].as_str().map(|u| json!({"type": "image_url", "image_url": {"url": u}})),
            _ => None,
        }).collect();
        if !parts.is_empty() {
            msgs.push(json!({"role": "user", "content": parts}));
        }
    }
    let mut out = json!({"model": model, "messages": msgs});
    if let Some(n) = req["max_tokens"].as_u64() { out["max_tokens"] = json!(n); }
    for k in ["temperature", "top_p"] {
        if !req[k].is_null() { out[k] = req[k].clone(); }
    }
    if let Some(s) = req["stop_sequences"].as_array() { out["stop"] = json!(s); }
    if let Some(tools) = req["tools"].as_array() {
        let fns: Vec<Value> = tools.iter().filter(|t| t["name"].is_string() && t.get("input_schema").is_some()).map(|t| json!({
            "type": "function", "function": {"name": t["name"], "description": t["description"], "parameters": t["input_schema"]}
        })).collect();
        if !fns.is_empty() { out["tools"] = json!(fns); }
    }
    // OpenAI refuses tool_choice without tools (all of them may have been server tools, which are dropped)
    if out.get("tools").is_some() { match req["tool_choice"]["type"].as_str() {
        Some("auto") => out["tool_choice"] = json!("auto"),
        Some("any") => out["tool_choice"] = json!("required"),
        Some("none") => out["tool_choice"] = json!("none"),
        Some("tool") => out["tool_choice"] = json!({"type": "function", "function": {"name": req["tool_choice"]["name"]}}),
        _ => {}
    } }
    if req["stream"] == true {
        out["stream"] = json!(true);
        out["stream_options"] = json!({"include_usage": true});
    }
    out
}

pub fn stop_reason(finish: &str) -> &'static str {
    match finish {
        "length" => "max_tokens",
        "tool_calls" | "function_call" => "tool_use",
        "content_filter" => "refusal",
        _ => "end_turn",
    }
}

/// OpenAI Chat Completions response -> Anthropic Messages response.
pub fn openai_to_anthropic(resp: &Value, model: &str) -> Value {
    let choice = &resp["choices"][0];
    let msg = &choice["message"];
    let mut content = vec![];
    if let Some(r) = reasoning_of(msg) {
        content.push(json!({"type": "thinking", "thinking": r, "signature": ""}));
    }
    if let Some(t) = msg["content"].as_str() {
        if !t.is_empty() { content.push(json!({"type": "text", "text": t})); }
    }
    for tc in msg["tool_calls"].as_array().into_iter().flatten() {
        let args = tc["function"]["arguments"].as_str().unwrap_or("{}");
        let id = tc["id"].as_str().filter(|s| !s.is_empty()).map(String::from).unwrap_or_else(|| format!("toolu_hrm_{}", content.len()));
        content.push(json!({"type": "tool_use", "id": id, "name": tc["function"]["name"],
            "input": serde_json::from_str::<Value>(args).unwrap_or(json!({}))}));
    }
    json!({
        "id": format!("msg_{}", resp["id"].as_str().unwrap_or("hrm")),
        "type": "message", "role": "assistant", "model": model, "content": content,
        "stop_reason": stop_reason(choice["finish_reason"].as_str().unwrap_or("stop")), "stop_sequence": null,
        "usage": {"input_tokens": resp["usage"]["prompt_tokens"].as_i64().unwrap_or(0), "output_tokens": resp["usage"]["completion_tokens"].as_i64().unwrap_or(0)}
    })
}

/// Reasoning text, which OpenAI-style providers put beside the answer: `reasoning` (OpenRouter, Ollama)
/// or `reasoning_content` (DeepSeek and others).
fn reasoning_of(v: &Value) -> Option<&str> {
    ["reasoning_content", "reasoning"].iter().find_map(|k| v[*k].as_str()).filter(|t| !t.is_empty())
}

fn sse(event: &str, data: Value) -> String { format!("event: {event}\ndata: {data}\n\n") }

/// Turns OpenAI stream chunks into Anthropic stream events.
pub struct StreamConv {
    model: String,
    started: bool,
    open: Option<usize>,
    /// "text" or "thinking" when the open block is one of those
    open_kind: &'static str,
    next: usize,
    /// Tool calls in order of first appearance: (OpenAI index, id, name, arguments). They are sent as whole
    /// blocks once complete: providers may interleave the chunks of parallel calls, and the id or name may
    /// arrive after the first chunk.
    tools: Vec<(u64, String, String, String)>,
    stop: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    done: bool,
    /// An error the provider reported inside the stream; the relay ends the answer with an error event.
    pub error: Option<String>,
}

impl StreamConv {
    pub fn new(model: &str) -> Self {
        StreamConv { model: model.into(), started: false, open: None, open_kind: "", next: 0, tools: vec![], stop: "end_turn".into(), input_tokens: 0, output_tokens: 0, done: false, error: None }
    }

    fn start(&mut self, out: &mut Vec<String>, id: &str) {
        if self.started { return; }
        self.started = true;
        out.push(sse("message_start", json!({"type": "message_start", "message": {
            "id": format!("msg_{id}"), "type": "message", "role": "assistant", "model": self.model, "content": [],
            "stop_reason": null, "stop_sequence": null, "usage": {"input_tokens": 0, "output_tokens": 0}}})));
    }

    fn close(&mut self, out: &mut Vec<String>) {
        self.open_kind = "";
        if let Some(i) = self.open.take() {
            out.push(sse("content_block_stop", json!({"type": "content_block_stop", "index": i})));
        }
    }

    /// The open block of this kind, or a new one after closing whatever is open.
    fn block(&mut self, out: &mut Vec<String>, kind: &'static str, start: Value) -> usize {
        if self.open_kind == kind { return self.open.unwrap(); }
        self.close(out);
        let i = self.next;
        self.next += 1;
        self.open = Some(i);
        self.open_kind = kind;
        out.push(sse("content_block_start", json!({"type": "content_block_start", "index": i, "content_block": start})));
        i
    }

    fn flush_tools(&mut self, out: &mut Vec<String>) {
        if self.tools.is_empty() { return; }
        self.close(out);
        for (_, id, name, args) in std::mem::take(&mut self.tools) {
            let i = self.next;
            self.next += 1;
            let id = if id.is_empty() { format!("toolu_hrm_{i}") } else { id };
            out.push(sse("content_block_start", json!({"type": "content_block_start", "index": i, "content_block": {"type": "tool_use", "id": id, "name": name, "input": {}}})));
            if !args.is_empty() {
                out.push(sse("content_block_delta", json!({"type": "content_block_delta", "index": i, "delta": {"type": "input_json_delta", "partial_json": args}})));
            }
            out.push(sse("content_block_stop", json!({"type": "content_block_stop", "index": i})));
        }
    }

    pub fn feed(&mut self, chunk: &Value) -> Vec<String> {
        let mut out = vec![];
        if self.done { return out; }
        // OpenRouter and others report a provider failure mid-stream as an error chunk or finish_reason "error"
        let failed = chunk["choices"][0]["finish_reason"] == "error";
        if chunk["error"].is_object() || failed {
            let msg = chunk["error"]["message"].as_str().unwrap_or("the provider reported an error").to_string();
            self.error = Some(msg);
            self.done = true;
            return out;
        }
        self.start(&mut out, chunk["id"].as_str().unwrap_or("hrm"));
        if let Some(u) = chunk["usage"].as_object() {
            self.input_tokens = u.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(self.input_tokens);
            self.output_tokens = u.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(self.output_tokens);
        }
        let Some(choice) = chunk["choices"].as_array().and_then(|c| c.first()) else { return out };
        let d = &choice["delta"];
        if let Some(t) = reasoning_of(d) {
            self.flush_tools(&mut out);
            let i = self.block(&mut out, "thinking", json!({"type": "thinking", "thinking": "", "signature": ""}));
            out.push(sse("content_block_delta", json!({"type": "content_block_delta", "index": i, "delta": {"type": "thinking_delta", "thinking": t}})));
        }
        if let Some(t) = d["content"].as_str().filter(|t| !t.is_empty()) {
            self.flush_tools(&mut out);
            let i = self.block(&mut out, "text", json!({"type": "text", "text": ""}));
            out.push(sse("content_block_delta", json!({"type": "content_block_delta", "index": i, "delta": {"type": "text_delta", "text": t}})));
        }
        for tc in d["tool_calls"].as_array().into_iter().flatten() {
            let oi = tc["index"].as_u64().unwrap_or(0);
            let pos = match self.tools.iter().position(|t| t.0 == oi) {
                Some(p) => p,
                None => { self.tools.push((oi, String::new(), String::new(), String::new())); self.tools.len() - 1 }
            };
            let t = &mut self.tools[pos];
            if t.1.is_empty() { if let Some(id) = tc["id"].as_str() { t.1 = id.into(); } }
            if t.2.is_empty() { if let Some(n) = tc["function"]["name"].as_str() { t.2 = n.into(); } }
            if let Some(a) = tc["function"]["arguments"].as_str() { t.3.push_str(a); }
        }
        if let Some(f) = choice["finish_reason"].as_str() {
            self.stop = stop_reason(f).into();
        }
        out
    }

    pub fn finish(&mut self) -> Vec<String> {
        let mut out = vec![];
        if self.done { return out; }
        self.done = true;
        self.start(&mut out, "hrm");
        self.flush_tools(&mut out);
        self.close(&mut out);
        out.push(sse("message_delta", json!({"type": "message_delta", "delta": {"stop_reason": self.stop, "stop_sequence": null},
            "usage": {"input_tokens": self.input_tokens, "output_tokens": self.output_tokens}})));
        out.push(sse("message_stop", json!({"type": "message_stop"})));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_error_chunk_is_not_a_finished_answer() {
        let mut c = StreamConv::new("m");
        c.feed(&json!({"id": "1", "choices": [{"index": 0, "delta": {"content": "Half an ans"}}]}));
        let out = c.feed(&json!({"id": "1", "error": {"message": "upstream overloaded", "code": 502}, "choices": [{"index": 0, "delta": {}, "finish_reason": "error"}]}));
        assert!(out.is_empty());
        assert_eq!(c.error.as_deref(), Some("upstream overloaded"));
        assert!(c.finish().is_empty(), "no message_stop after an error");
    }

    #[test]
    fn request_maps_system_tools_and_results() {
        let req = json!({"model": "claude-x", "max_tokens": 100, "system": [{"type": "text", "text": "Be brief"}], "stream": true,
            "tools": [{"name": "ls", "description": "list", "input_schema": {"type": "object"}}], "tool_choice": {"type": "any"},
            "messages": [
                {"role": "user", "content": "hi"},
                {"role": "assistant", "content": [{"type": "text", "text": "calling"}, {"type": "tool_use", "id": "t1", "name": "ls", "input": {"p": "."}}]},
                {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t1", "content": "a.txt"}, {"type": "text", "text": "and?"}]}]});
        let o = anthropic_to_openai(&req, "gpt-x");
        assert_eq!(o["model"], "gpt-x");
        assert_eq!(o["messages"][0], json!({"role": "system", "content": "Be brief"}));
        assert_eq!(o["messages"][2]["tool_calls"][0]["function"]["arguments"], "{\"p\":\".\"}");
        assert_eq!(o["messages"][3], json!({"role": "tool", "tool_call_id": "t1", "content": "a.txt"}));
        assert_eq!(o["messages"][4]["content"][0]["text"], "and?");
        assert_eq!(o["tools"][0]["function"]["name"], "ls");
        assert_eq!(o["tool_choice"], "required");
        assert_eq!(o["stream_options"]["include_usage"], true);
    }

    #[test]
    fn response_maps_text_tools_and_usage() {
        let r = json!({"id": "c1", "choices": [{"finish_reason": "tool_calls", "message": {"content": "ok", "tool_calls": [{"id": "x", "function": {"name": "ls", "arguments": "{\"p\":1}"}}]}}],
            "usage": {"prompt_tokens": 7, "completion_tokens": 3}});
        let a = openai_to_anthropic(&r, "m");
        assert_eq!(a["content"][0]["text"], "ok");
        assert_eq!(a["content"][1]["input"]["p"], 1);
        assert_eq!(a["stop_reason"], "tool_use");
        assert_eq!(a["usage"]["output_tokens"], 3);
    }

    #[test]
    fn reasoning_becomes_a_thinking_block() {
        let r = json!({"id": "c2", "choices": [{"finish_reason": "stop", "message": {"content": "router works", "reasoning": "The user wants two words."}}], "usage": {}});
        let a = openai_to_anthropic(&r, "m");
        assert_eq!(a["content"][0], json!({"type": "thinking", "thinking": "The user wants two words.", "signature": ""}));
        assert_eq!(a["content"][1]["text"], "router works");
        let mut c = StreamConv::new("m");
        let mut ev = vec![];
        ev.extend(c.feed(&json!({"id": "s", "choices": [{"delta": {"reasoning_content": "Count"}}]})));
        ev.extend(c.feed(&json!({"id": "s", "choices": [{"delta": {"reasoning_content": "ing"}}]})));
        ev.extend(c.feed(&json!({"id": "s", "choices": [{"delta": {"content": "12345"}}]})));
        ev.extend(c.finish());
        let all = ev.join("");
        assert_eq!(all.matches("event: content_block_start").count(), 2, "one thinking block, then one text block");
        assert!(all.find("thinking_delta").unwrap() < all.find("text_delta").unwrap());
    }

    #[test]
    fn stream_produces_ordered_anthropic_events() {
        let mut c = StreamConv::new("m");
        let mut ev: Vec<String> = vec![];
        ev.extend(c.feed(&json!({"id": "s", "choices": [{"delta": {"content": "He"}}]})));
        ev.extend(c.feed(&json!({"id": "s", "choices": [{"delta": {"content": "llo"}}]})));
        ev.extend(c.feed(&json!({"id": "s", "choices": [{"delta": {"tool_calls": [{"index": 0, "id": "t", "function": {"name": "ls", "arguments": "{\"a\""}}]}}]})));
        ev.extend(c.feed(&json!({"id": "s", "choices": [{"delta": {"tool_calls": [{"index": 0, "function": {"arguments": ":1}"}}]}, "finish_reason": "tool_calls"}]})));
        ev.extend(c.feed(&json!({"id": "s", "choices": [], "usage": {"prompt_tokens": 5, "completion_tokens": 2}})));
        ev.extend(c.finish());
        let kinds: Vec<&str> = ev.iter().map(|e| e.lines().next().unwrap().trim_start_matches("event: ")).collect();
        assert_eq!(kinds, vec!["message_start", "content_block_start", "content_block_delta", "content_block_delta", "content_block_stop",
            "content_block_start", "content_block_delta", "content_block_stop", "message_delta", "message_stop"]);
        assert!(ev[6].contains(r#"{\"a\":1}"#), "{}", ev[6]);
        assert!(ev[8].contains("\"tool_use\"") && ev[8].contains("\"output_tokens\":2"));
    }

    #[test]
    fn thinking_only_turns_are_dropped_and_tool_result_images_kept() {
        let img = json!({"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "AAA"}});
        let req = json!({"messages": [
            {"role": "user", "content": "go"},
            {"role": "assistant", "content": [{"type": "thinking", "thinking": "hmm", "signature": ""}]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t1", "content": [{"type": "text", "text": "shot"}, img]}]}]});
        let o = anthropic_to_openai(&req, "m");
        let msgs = o["messages"].as_array().unwrap();
        assert!(!msgs.iter().any(|m| m["role"] == "assistant"), "a thinking-only turn has nothing OpenAI accepts");
        assert_eq!(msgs.last().unwrap()["content"][0]["image_url"]["url"], "data:image/png;base64,AAA");
    }

    #[test]
    fn interleaved_parallel_tool_calls_stay_apart() {
        let mut c = StreamConv::new("m");
        let mut ev: Vec<String> = vec![];
        // two calls whose chunks alternate; the second call's id arrives late and the first never gets one
        for (i, id, name, args) in [(0, "", "ls", "{\"p\":"), (1, "", "", "{\"q\":"), (0, "", "", "1}"), (1, "call_b", "cat", "2}")] {
            ev.extend(c.feed(&json!({"id": "s", "choices": [{"delta": {"tool_calls": [{"index": i, "id": id, "function": {"name": name, "arguments": args}}]}}]})));
        }
        ev.extend(c.finish());
        let starts: Vec<&String> = ev.iter().filter(|e| e.starts_with("event: content_block_start")).collect();
        assert_eq!(starts.len(), 2);
        assert!(starts[0].contains("\"name\":\"ls\"") && starts[0].contains("toolu_hrm_0"));
        assert!(starts[1].contains("\"name\":\"cat\"") && starts[1].contains("call_b"));
        let deltas: Vec<&String> = ev.iter().filter(|e| e.starts_with("event: content_block_delta")).collect();
        assert!(deltas[0].contains(r#"{\"p\":1}"#) && deltas[1].contains(r#"{\"q\":2}"#));
    }
}
