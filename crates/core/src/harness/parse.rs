//! Shared parsing helpers: timestamps, incremental JSONL reading, one-line summaries.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::Result;
use serde_json::Value;

use crate::model::SessionMeta;
use crate::util::{file_sig, truncate};
use crate::Ctx;

/// Run `work` on every file using all cores, biggest files first, and hand each result to `store` (on this
/// thread, one at a time) as soon as it is ready. Reading and parsing are the slow part: gigabytes of logs on a
/// first run. Each file's rows and cursor travel together, so its rows are still written before its cursor.
pub fn each_file_parallel<T: Send>(files: &[std::path::PathBuf], work: impl Fn(&Path) -> T + Sync, store: impl FnMut(T) -> Result<()>) -> Result<()> {
    let sized: Vec<(std::path::PathBuf, i64, u64)> = files.iter().map(|f| (f.clone(), 0, std::fs::metadata(f).map(|m| m.len()).unwrap_or(0))).collect();
    each_file_parallel_sig(&sized, |f, _, _| work(f), store)
}

/// `each_file_parallel` for files whose size and time the walk already gave: no metadata call per file.
pub fn each_file_parallel_sig<T: Send>(files: &[(std::path::PathBuf, i64, u64)], work: impl Fn(&Path, i64, u64) -> T + Sync, mut store: impl FnMut(T) -> Result<()>) -> Result<()> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut order: Vec<usize> = (0..files.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(files[i].2));
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(16).min(files.len().max(1));
    let next = AtomicUsize::new(0);
    let (tx, rx) = std::sync::mpsc::sync_channel::<T>(n * 2);
    std::thread::scope(|s| {
        for _ in 0..n {
            let (tx, next, work, order) = (tx.clone(), &next, &work, &order);
            s.spawn(move || loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                let Some(&i) = order.get(k) else { break };
                // a failed store dropped the receiver: stop instead of parsing the rest for nothing
                let (p, m, l) = &files[i];
                if tx.send(work(p, *m, *l)).is_err() { break; }
            });
        }
        drop(tx);
        let mut res = Ok(());
        for t in rx.iter() {
            if let Err(e) = store(t) { res = Err(e); break; }
        }
        drop(rx);
        res
    })
}

pub fn ts_ms(s: &str) -> i64 { chrono::DateTime::parse_from_rfc3339(s).map(|d| d.timestamp_millis()).unwrap_or(0) }

/// The `"timestamp":"..."` of a log line without parsing the whole line (tool results can be megabytes).
pub fn line_ts(line: &str) -> Option<i64> {
    let i = line.find("\"timestamp\":\"")? + 13;
    let end = line[i..].find('"')? + i;
    Some(ts_ms(&line[i..end])).filter(|t| *t > 0)
}

#[cfg(test)]
mod line_ts_tests {
    #[test]
    fn reads_the_timestamp_without_parsing() {
        assert_eq!(super::line_ts(r#"{"type":"user","timestamp":"2026-11-01T10:00:00.500Z","x":1}"#), Some(1793527200500));
        assert_eq!(super::line_ts(r#"{"type":"user"}"#), None);
    }
}

pub fn str_at<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut cur = v;
    for k in path {
        cur = cur.get(k)?;
    }
    cur.as_str()
}

pub fn i64_at(v: &Value, path: &[&str]) -> i64 {
    let mut cur = v;
    for k in path {
        match cur.get(k) {
            Some(x) => cur = x,
            None => return 0,
        }
    }
    cur.as_i64().unwrap_or(0)
}

/// Whole file as text; `.zst` files are decompressed (Codex stores rollouts that way).
pub fn read_text(path: &Path) -> Result<String> {
    let raw = std::fs::read(path)?;
    let bytes = if path.extension().map(|e| e == "zst").unwrap_or(false) { zstd::decode_all(&raw[..])? } else { raw };
    Ok(String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()))
}

/// The first line of a log, reading only as much as needed (`.zst` decompressed as it is read).
pub fn first_line(path: &Path) -> Option<String> {
    use std::io::BufRead;
    let f = std::fs::File::open(path).ok()?;
    let r: Box<dyn Read> = if path.extension().map(|e| e == "zst").unwrap_or(false) { Box::new(zstd::Decoder::new(f).ok()?) } else { Box::new(f) };
    std::io::BufReader::new(r).lines().next()?.ok()
}

/// Where an import got to in one file. Saved with `save` only after that file's rows are stored, so an
/// interrupted import reads the file again instead of skipping rows it never stored.
pub struct NewCursor { key: String, profile: String, offset: i64, mtime: i64, len: i64, tail: String }

impl NewCursor {
    pub fn save(self, ctx: &Ctx) -> Result<()> { ctx.db.set_cursor(&self.key, &self.profile, self.offset, self.mtime, self.len, &self.tail) }
}

/// Feed the complete lines added to `path` since the last saved cursor to `f`; returns the cursor to save.
/// A file that shrank or whose bytes before the cursor changed is read again from the start; usage ids
/// are deduplicated, so a re-read never double counts. Compressed files are re-read whole when their size
/// or time changes.
pub fn for_new_lines(ctx: &Ctx, profile: &str, path: &Path, f: impl FnMut(&str)) -> Result<Option<NewCursor>> {
    let cur = ctx.db.cursor(&path.display().to_string())?;
    for_new_lines_with(ctx, profile, path, cur, f)
}

/// `for_new_lines` with the cursor already looked up (an import fetches all of an account's cursors in one query).
pub fn for_new_lines_with(ctx: &Ctx, profile: &str, path: &Path, cur: Option<(i64, i64, i64, String)>, f: impl FnMut(&str)) -> Result<Option<NewCursor>> {
    let Some((mtime, len)) = file_sig(path) else { return Ok(None) };
    for_new_lines_sig(ctx, profile, path, mtime, len, cur, f)
}

/// A changed session's last cached record, dated by the file: shown now while the file is parsed again later.
/// Set while the index refreshes: a changed file is parsed now, on the refresh's thread, not later.
pub static PARSE_NOW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn parse_now() -> bool { PARSE_NOW.load(std::sync::atomic::Ordering::Relaxed) }

pub fn stale(meta: &SessionMeta, mtime: i64) -> SessionMeta {
    let mut m = meta.clone();
    m.last_active_at = m.last_active_at.max(mtime);
    m
}

/// Parse a session file on a background thread and refresh its cached record; a file already being parsed is
/// left alone. A page open then costs no parse of the session still being written (145 MB rollouts were seen).
pub fn refresh_session_later(ctx: &Ctx, key: String, path: std::path::PathBuf, mtime: i64, len: u64, parse: impl Fn(&Path) -> Option<SessionMeta> + Send + 'static) {
    static BUSY: std::sync::Mutex<Option<std::collections::HashSet<String>>> = std::sync::Mutex::new(None);
    { let mut b = BUSY.lock().unwrap_or_else(|e| e.into_inner()); if !b.get_or_insert_with(Default::default).insert(key.clone()) { return; } }
    let (data, home) = (ctx.data.clone(), ctx.home.clone());
    std::thread::spawn(move || {
        if let (Some(meta), Ok(ctx)) = (parse(&path), Ctx::open_at(&data, &home)) {
            if meta.started_at != 0 || !meta.id.is_empty() { let _ = ctx.db.put_session(&key, &meta, mtime, len as i64); }
        }
        BUSY.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(Default::default).remove(&key);
    });
}

/// `for_new_lines_with` with the file's size and time already known (from the directory listing).
pub fn for_new_lines_sig(_ctx: &Ctx, profile: &str, path: &Path, mtime: i64, len: u64, cur: Option<(i64, i64, i64, String)>, mut f: impl FnMut(&str)) -> Result<Option<NewCursor>> {
    let key = path.display().to_string();
    let compressed = path.extension().map(|e| e == "zst").unwrap_or(false);
    if let Some((_, m, l, _)) = &cur {
        if *m == mtime && *l == len as i64 {
            return Ok(None);
        }
    }
    if compressed {
        for line in read_text(path)?.lines() {
            f(line);
        }
        return Ok(Some(NewCursor { key, profile: profile.into(), offset: len as i64, mtime, len: len as i64, tail: String::new() }));
    }
    let mut file = std::fs::File::open(path)?;
    let mut start = 0u64;
    if let Some((off, _, _, tail)) = cur {
        let off = off as u64;
        if off <= len && off >= 32 {
            let mut buf = [0u8; 32];
            file.seek(SeekFrom::Start(off - 32))?;
            file.read_exact(&mut buf)?;
            if hex::encode(buf) == tail {
                start = off;
            }
        }
    }
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    // only complete lines; a half-written last line waits for the next import
    let end = bytes.iter().rposition(|b| *b == b'\n').map(|i| i + 1).unwrap_or(0);
    for line in String::from_utf8_lossy(&bytes[..end]).lines() {
        f(line);
    }
    let new_off = start + end as u64;
    let tail = if new_off >= 32 {
        let mut buf = [0u8; 32];
        file.seek(SeekFrom::Start(new_off - 32))?;
        file.read_exact(&mut buf)?;
        hex::encode(buf)
    } else {
        String::new()
    };
    Ok(Some(NewCursor { key, profile: profile.into(), offset: new_off as i64, mtime, len: len as i64, tail }))
}

/// One-line description of a tool call's input for the step list.
pub fn summarize_input(tool: &str, input: &Value) -> String {
    let pick = |keys: &[&str]| keys.iter().find_map(|k| input.get(*k).and_then(|v| v.as_str())).map(|s| s.to_string());
    let s = match tool {
        "Bash" | "PowerShell" | "exec_command" | "shell" | "bash" => pick(&["command", "cmd"]).or_else(|| {
            input.get("command").and_then(|c| c.as_array()).map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" "))
        }),
        "Read" | "Edit" | "Write" | "MultiEdit" | "NotebookEdit" | "read" | "edit" | "write" => pick(&["file_path", "filePath", "path", "notebook_path"]),
        "Grep" | "Glob" | "grep" | "glob" => pick(&["pattern"]),
        "WebFetch" | "webfetch" => pick(&["url"]),
        "WebSearch" | "websearch" => pick(&["query"]),
        "Task" | "Agent" | "task" => pick(&["description", "prompt"]),
        _ => None,
    };
    let s = s.unwrap_or_else(|| match input {
        Value::String(x) => x.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    });
    truncate(&s.replace(['\n', '\r'], " "), 160)
}

/// One-line description of a tool result: its only non-empty line, or the line count when there are several.
pub fn summarize_output(text: &str) -> String {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    match lines.len() {
        0 => String::new(),
        1 => truncate(lines[0], 80),
        n => format!("{n} lines"),
    }
}

/// Text of a content value that may be a string or an array of `{type:"text", text}` blocks.
pub fn content_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .filter_map(|b| b.get("text").and_then(|t| t.as_str()).or_else(|| b.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Count added and removed lines in a unified diff body.
pub fn diff_counts(diff: &str) -> (u32, u32) {
    let mut a = 0;
    let mut r = 0;
    for l in diff.lines() {
        // no file headers reach here (diffs are built from hunks), so "---" is a removed "--" line
        if l.starts_with('+') { a += 1 }
        if l.starts_with('-') { r += 1 }
    }
    (a, r)
}
