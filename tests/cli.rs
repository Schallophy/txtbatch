use std::fs;
use std::process::{Command, Output};

use tempfile::tempdir;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_txtbatch")
}

fn run_in(appdata: &std::path::Path, args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .env("APPDATA", appdata)
        .output()
        .expect("运行 txtbatch 失败")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn canonical_display(p: &std::path::Path) -> String {
    let c = fs::canonicalize(p).expect("canonicalize 失败");
    let s = c.to_string_lossy().to_string();
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s).to_string();
    s
}

#[test]
fn help_lists_commands() {
    let out = Command::new(bin()).arg("--help").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    for cmd in ["replace", "insert", "set-dir", "show-dir", "clear-dir", "--dir"] {
        assert!(text.contains(cmd), "帮助信息缺少 `{cmd}`");
    }
    for en in ["Usage:", "Commands:", "Options:", "Print help", "Print version"] {
        assert!(text.contains(en), "帮助信息缺少标题 `{en}`");
    }
    assert!(text.contains("help       Print this message"), "帮助信息缺少 help 子命令");
}

fn help_prefix_width(line: &str, needle: &str) -> usize {
    let idx = line.find(needle).expect("描述文本缺失");
    line[..idx]
        .chars()
        .map(|c| if c as u32 > 0x2E80 { 2 } else { 1 })
        .sum()
}

#[test]
fn help_options_aligned() {
    let out = Command::new(bin()).arg("--help").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    let lines: Vec<&str> = text
        .lines()
        .skip_while(|l| !l.starts_with("Options:"))
        .skip(1)
        .take_while(|l| !l.trim().is_empty())
        .collect();

    let expect = [
        ("--dir", "本次操作"),
        ("--dry-run", "仅预览"),
        ("--diff", "显示每一处"),
    ];

    let mut cols = Vec::new();
    for (opt, desc) in expect {
        if let Some(line) = lines.iter().find(|l| l.contains(opt)) {
            cols.push(help_prefix_width(line, desc));
        }
    }
    assert_eq!(cols.len(), 3, "应找到全部 3 个选项，实际 {cols:?}");
    for c in &cols {
        assert_eq!(*c, cols[0], "选项描述列宽未对齐: {cols:?}");
    }
}

#[test]
fn no_dir_and_no_saved_dir_errors() {
    let cfg = tempdir().unwrap();
    let out = run_in(cfg.path(), &["replace", "foo", "bar"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("默认目录"));
}

#[test]
fn empty_find_errors() {
    let cfg = tempdir().unwrap();
    let work = tempdir().unwrap();
    let out = run_in(cfg.path(), &["--dir", work.path().to_str().unwrap(), "replace", "", "x"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("查找文本不能为空"));
}

#[test]
fn set_dir_then_run_without_dir() {
    let cfg = tempdir().unwrap();
    let work = tempdir().unwrap();
    let target = work.path().join("a.txt");
    fs::write(&target, "foo bar").unwrap();
    let dir = work.path().to_str().unwrap();

    let set = run_in(cfg.path(), &["set-dir", dir]);
    assert_eq!(set.status.code(), Some(0));
    assert!(stdout(&set).contains("已设置默认目录"));

    let show = run_in(cfg.path(), &["show-dir"]);
    assert!(stdout(&show).contains("当前默认目录"));

    let run = run_in(cfg.path(), &["replace", "foo", "FOO"]);
    assert_eq!(run.status.code(), Some(0));
    assert!(stdout(&run).contains(&format!("目录: {}", canonical_display(work.path()))));
    assert!(stdout(&run).contains("共 1 处替换"));
    assert_eq!(fs::read_to_string(&target).unwrap(), "FOO bar");
}

#[test]
fn dir_flag_persists_and_shows() {
    let cfg = tempdir().unwrap();
    let work = tempdir().unwrap();
    let target = work.path().join("a.txt");
    fs::write(&target, "foo").unwrap();
    let dir = work.path().to_str().unwrap();

    let out = run_in(cfg.path(), &["--dir", dir, "replace", "foo", "x", "--dry-run"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(stdout(&out).contains("[dry-run]"));
    assert_eq!(fs::read_to_string(&target).unwrap(), "foo");

    let show = run_in(cfg.path(), &["show-dir"]);
    assert!(stdout(&show).contains(&canonical_display(work.path())));
}

#[test]
fn show_dir_when_unset() {
    let cfg = tempdir().unwrap();
    let out = run_in(cfg.path(), &["show-dir"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(stdout(&out).contains("尚未设置默认目录"));
}

#[test]
fn clear_dir_resets() {
    let cfg = tempdir().unwrap();
    let work = tempdir().unwrap();
    run_in(cfg.path(), &["set-dir", work.path().to_str().unwrap()]);
    let out = run_in(cfg.path(), &["clear-dir"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(stdout(&out).contains("已清除默认目录"));
    let show = run_in(cfg.path(), &["show-dir"]);
    assert!(stdout(&show).contains("尚未设置默认目录"));
}

#[test]
fn insert_end_to_end_via_cli() {
    let cfg = tempdir().unwrap();
    let work = tempdir().unwrap();
    let target = work.path().join("a.txt");
    fs::write(&target, "你好世界").unwrap();
    let dir = work.path().to_str().unwrap();

    let out = run_in(cfg.path(), &["--dir", dir, "insert", "你好", " !"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(stdout(&out).contains("共 1 处插入"));
    assert_eq!(fs::read_to_string(&target).unwrap(), "你好 !世界");
}

#[test]
fn nonexistent_dir_errors() {
    let cfg = tempdir().unwrap();
    let missing = tempdir().unwrap().path().join("nope");
    let out = run_in(
        cfg.path(),
        &["--dir", missing.to_str().unwrap(), "replace", "a", "b"],
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("目录不存在"));
}