use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use colored::Colorize;
use memchr::memmem;
use walkdir::WalkDir;

pub mod config;
pub mod gui;

pub const DEFAULT_CTX: usize = 24;

/// 单个编辑：在原始字节中 [start, end) 区间被 replacement 替换。
/// 插入操作中 start == end，replacement 为要插入的字节。
#[derive(Debug, Clone)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub replacement: Vec<u8>,
}

/// 单处改动对应的前后行文本（已做截断处理）
#[derive(Debug, Clone)]
pub struct LineDiff {
    pub line_no: usize,
    pub start: String,
    pub new: String,
}

#[derive(Debug, Clone)]
pub struct FileEdit {
    pub path: PathBuf,
    pub diffs: Vec<LineDiff>,
}

#[derive(Debug, Clone)]
pub enum Operation {
    Replace { find: String, replace: String },
    Insert { after: String, insert: String },
}

impl Operation {
    pub fn name(&self) -> &'static str {
        match self {
            Operation::Replace { .. } => "替换",
            Operation::Insert { .. } => "插入",
        }
    }

    pub fn edits_for(&self, data: &[u8]) -> Vec<Edit> {
        match self {
            Operation::Replace { find, replace } => {
                let f = find.as_bytes();
                let r = replace.as_bytes();
                memmem::find_iter(data, f)
                    .map(|m| Edit {
                        start: m,
                        end: m + f.len(),
                        replacement: r.to_vec(),
                    })
                    .collect()
            }
            Operation::Insert { after, insert } => {
                let a = after.as_bytes();
                let i = insert.as_bytes();
                memmem::find_iter(data, a)
                    .map(|m| Edit {
                        start: m + a.len(),
                        end: m + a.len(),
                        replacement: i.to_vec(),
                    })
                    .collect()
            }
        }
    }
}

/// 依序应用互不重叠的编辑，返回新字节与编辑处数
pub fn apply_edits(data: &[u8], edits: &[Edit]) -> (Vec<u8>, usize) {
    let mut out = Vec::with_capacity(data.len());
    let mut cursor = 0usize;
    for e in edits {
        out.extend_from_slice(&data[cursor..e.start]);
        out.extend_from_slice(&e.replacement);
        cursor = e.end;
    }
    out.extend_from_slice(&data[cursor..]);
    (out, edits.len())
}

/// 为每一处编辑生成 diff 风格的前后文本（行内截断，含行号）
pub fn build_diffs(data: &[u8], edits: &[Edit], ctx: usize) -> Vec<LineDiff> {
    let mut out = Vec::with_capacity(edits.len());
    for e in edits {
        let line_no = data[..e.start].iter().filter(|&&b| b == b'\n').count() + 1;
        let line_start = data[..e.start]
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |i| i + 1);
        let mut line_end = match data[e.start..].iter().position(|&b| b == b'\n') {
            Some(i) => e.start + i,
            None => data.len(),
        };
        if line_end > line_start && data[line_end - 1] == b'\r' {
            line_end -= 1;
        }

        let before_avail = e.start - line_start;
        let after_avail = line_end.saturating_sub(e.end);
        let mut bs = e.start - before_avail.min(ctx);
        let mut ae = e.end + after_avail.min(ctx);

        while bs < line_end && (data[bs] & 0xC0) == 0x80 {
            bs += 1;
        }
        while ae < data.len() && ae < line_end && (data[ae] & 0xC0) == 0x80 {
            ae += 1;
        }

        let has_lead = before_avail > ctx;
        let has_tail = after_avail > ctx;

        let mut old_bytes = Vec::new();
        if has_lead {
            old_bytes.extend_from_slice("\u{2026}".as_bytes());
        }
        old_bytes.extend_from_slice(&data[bs..e.start]);
        old_bytes.extend_from_slice(&data[e.start..e.end.min(data.len())]);
        old_bytes.extend_from_slice(&data[e.end.min(data.len())..ae]);
        if has_tail {
            old_bytes.extend_from_slice("\u{2026}".as_bytes());
        }

        let mut new_bytes = Vec::new();
        if has_lead {
            new_bytes.extend_from_slice("\u{2026}".as_bytes());
        }
        new_bytes.extend_from_slice(&data[bs..e.start]);
        new_bytes.extend_from_slice(&e.replacement);
        new_bytes.extend_from_slice(&data[e.end.min(data.len())..ae]);
        if has_tail {
            new_bytes.extend_from_slice("\u{2026}".as_bytes());
        }

        out.push(LineDiff {
            line_no,
            start: String::from_utf8_lossy(&old_bytes).into_owned(),
            new: String::from_utf8_lossy(&new_bytes).into_owned(),
        });
    }
    out
}

#[derive(Debug, Default)]
pub struct Summary {
    pub files_scanned: usize,
    pub files_modified: usize,
    pub total_edits: usize,
    pub binary_skipped: usize,
    pub modified: Vec<PathBuf>,
    pub details: Vec<FileEdit>,
    pub unmatched: Vec<PathBuf>,
    pub errors: Vec<(PathBuf, String)>,
}

pub fn process_dir(dir: &Path, op: &Operation, dry_run: bool, ctx: usize) -> Result<Summary> {
    if !dir.exists() {
        bail!("目录不存在: {}", dir.display());
    }
    if !dir.is_dir() {
        bail!("不是目录: {}", dir.display());
    }

    let mut summary = Summary::default();
    let walker = WalkDir::new(dir).sort_by_file_name();
    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                summary
                    .errors
                    .push((PathBuf::new(), format!("遍历失败: {e}")));
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        summary.files_scanned += 1;

        let data = match fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                summary.errors.push((path.to_path_buf(), format!("读取失败: {e}")));
                continue;
            }
        };

        if is_binary(&data) {
            summary.binary_skipped += 1;
            continue;
        }

        let edits = op.edits_for(&data);
        if edits.is_empty() {
            summary.unmatched.push(path.to_path_buf());
            continue;
        }
        let (new_data, count) = apply_edits(&data, &edits);

        summary.files_modified += 1;
        summary.total_edits += count;
        summary.modified.push(path.to_path_buf());
        summary.details.push(FileEdit {
            path: path.to_path_buf(),
            diffs: build_diffs(&data, &edits, ctx),
        });

        if !dry_run {
            if let Err(e) = fs::write(path, &new_data) {
                summary
                    .errors
                    .push((path.to_path_buf(), format!("写入失败: {e}")));
            }
        }
    }

    Ok(summary)
}

pub fn replace_all_bytes(data: &[u8], find: &[u8], replace: &[u8]) -> (Vec<u8>, usize) {
    if find.is_empty() {
        return (data.to_vec(), 0);
    }
    let edits: Vec<Edit> = memmem::find_iter(data, find)
        .map(|m| Edit {
            start: m,
            end: m + find.len(),
            replacement: replace.to_vec(),
        })
        .collect();
    apply_edits(data, &edits)
}

pub fn insert_after_all_bytes(data: &[u8], after: &[u8], insert: &[u8]) -> (Vec<u8>, usize) {
    if after.is_empty() {
        return (data.to_vec(), 0);
    }
    let edits: Vec<Edit> = memmem::find_iter(data, after)
        .map(|m| Edit {
            start: m + after.len(),
            end: m + after.len(),
            replacement: insert.to_vec(),
        })
        .collect();
    apply_edits(data, &edits)
}

pub fn is_binary(data: &[u8]) -> bool {
    let probe = &data[..data.len().min(8000)];
    probe.contains(&0)
}

pub fn canonical_clean(path: &Path) -> PathBuf {
    let canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = canon.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        canon
    }
}

pub fn run_edit(dir_arg: Option<PathBuf>, op: Operation, dry_run: bool, show_diff: bool) -> Result<()> {
    let dir = match dir_arg {
        Some(d) => {
            let canonical = canonical_clean(&d);
            if let Err(e) = config::save_dir(&canonical.to_string_lossy()) {
                eprintln!("警告：无法保存默认目录: {e}");
            }
            canonical
        }
        None => match config::load_dir()? {
            Some(saved) => PathBuf::from(saved),
            None => bail!(
                "未指定目录，也没有已保存的默认目录。\n请用 `txtbatch --dir <目录> ...` 或先运行 `txtbatch set-dir <目录>`"
            ),
        },
    };

    println!("目录: {}", dir.display());
    let summary = process_dir(&dir, &op, dry_run, DEFAULT_CTX)?;

    if dry_run {
        println!("[dry-run] 以下为预览，未写入任何文件");
    }
    let verb = match &op {
        Operation::Replace { .. } => "替换",
        Operation::Insert { .. } => "插入",
    };
    println!(
        "扫描文件 {count} 个，修改 {modified} 个，共 {edits} 处{verb}",
        count = summary.files_scanned,
        modified = summary.files_modified,
        edits = summary.total_edits
    );
    if summary.binary_skipped > 0 {
        println!("跳过二进制文件 {} 个", summary.binary_skipped);
    }
    if !summary.modified.is_empty() {
        if show_diff {
            for f in &summary.details {
                println!(
                    "文件: {}（{} 处）",
                    f.path.display(),
                    f.diffs.len()
                );
                for d in &f.diffs {
                    println!("第 {} 行", d.line_no);
                    println!("  {}", format!("- {}", d.start).red());
                    println!("  {}", format!("+ {}", d.new).green());
                }
            }
        } else {
            println!("修改的文件:");
            for f in &summary.details {
                println!("  {}（{} 处）", f.path.display(), f.diffs.len());
            }
        }
    }
    if !summary.unmatched.is_empty() {
        println!("未找到匹配的文件 {} 个", summary.unmatched.len());
    }
    for (p, err) in &summary.errors {
        println!("错误[{}]: {err}", p.display());
    }

    Ok(())
}

pub fn load_cjk_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    let font_paths = [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
        r"C:\Windows\Fonts\simsun.ttc",
        r"C:\Windows\Fonts\msyhbd.ttc",
    ];

    for path in &font_paths {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert(
                "cjk".to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(data)),
            );
            if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                family.push("cjk".to_owned());
            }
            if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                family.push("cjk".to_owned());
            }
            ctx.set_fonts(fonts);
            return;
        }
    }

    eprintln!("警告：未找到中文字体，中文可能显示为方块");
}

pub fn launch_gui() -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([600.0, 400.0])
            .with_title("txtbatch - 批量文本工具"),
        ..Default::default()
    };
    eframe::run_native(
        "txtbatch",
        options,
        Box::new(|cc| {
            load_cjk_font(&cc.egui_ctx);
            Ok(Box::new(gui::App::default()))
        }),
    )
    .map_err(|e| anyhow::anyhow!("GUI 启动失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_basic() {
        let (out, n) = replace_all_bytes(b"hello world hello", b"hello", b"hi");
        assert_eq!(out, b"hi world hi");
        assert_eq!(n, 2);
    }

    #[test]
    fn replace_chinese_bytes() {
        let (out, n) = replace_all_bytes("你好世界".as_bytes(), "世界".as_bytes(), "中国".as_bytes());
        assert_eq!(out, "你好中国".as_bytes());
        assert_eq!(n, 1);
    }

    #[test]
    fn replace_none() {
        let (out, n) = replace_all_bytes(b"abc", b"xyz", b"1");
        assert_eq!(out, b"abc");
        assert_eq!(n, 0);
    }

    #[test]
    fn replace_empty_find_no_op() {
        let (out, n) = replace_all_bytes(b"abc", b"", b"x");
        assert_eq!(out, b"abc");
        assert_eq!(n, 0);
    }

    #[test]
    fn replace_utf8_bom_preserved() {
        let (out, n) = replace_all_bytes(b"\xef\xbb\xbfabc", b"abc", b"def");
        assert_eq!(out, b"\xef\xbb\xbfdef");
        assert_eq!(n, 1);
    }

    #[test]
    fn replace_gbk_bytes_no_utf8_decode() {
        let (out, n) = replace_all_bytes(b"\xc4\xe3\xba\xc3", b"\xba\xc3", b"\xb2\xbb\xb4\xed");
        assert_eq!(out, b"\xc4\xe3\xb2\xbb\xb4\xed");
        assert_eq!(n, 1);
    }

    #[test]
    fn insert_basic() {
        let (out, n) = insert_after_all_bytes(b"a,b", b",", b" x");
        assert_eq!(out, b"a, xb");
        assert_eq!(n, 1);
    }

    #[test]
    fn insert_multiple() {
        let (out, n) = insert_after_all_bytes(b"abab", b"ab", b"-");
        assert_eq!(out, b"ab-ab-");
        assert_eq!(n, 2);
    }

    #[test]
    fn insert_none() {
        let (out, n) = insert_after_all_bytes(b"abc", b"z", b"!");
        assert_eq!(out, b"abc");
        assert_eq!(n, 0);
    }

    #[test]
    fn insert_adjacent_no_overlap() {
        let (out, n) = insert_after_all_bytes(b"aaa", b"aa", b"!");
        assert_eq!(out, b"aa!a");
        assert_eq!(n, 1);
    }

    #[test]
    fn binary_detection() {
        assert!(is_binary(b"a\x00b"));
        assert!(is_binary(&[0u8; 10]));
        assert!(!is_binary(b"plain text"));
    }

    #[test]
    fn operation_apply_roundtrip() {
        let op = Operation::Replace {
            find: "foo".into(),
            replace: "bar".into(),
        };
        let edits = op.edits_for(b"foo foo");
        let (out, n) = apply_edits(b"foo foo", &edits);
        assert_eq!(out, b"bar bar");
        assert_eq!(n, 2);

        let op = Operation::Insert {
            after: "foo".into(),
            insert: "!".into(),
        };
        let edits = op.edits_for(b"foo");
        let (out, n) = apply_edits(b"foo", &edits);
        assert_eq!(out, b"foo!");
        assert_eq!(n, 1);
    }

    #[test]
    fn edits_for_matches_count_and_spans() {
        let op = Operation::Replace {
            find: "ab".into(),
            replace: "X".into(),
        };
        let edits = op.edits_for(b"ab X ab");
        assert_eq!(edits.len(), 2);
        assert_eq!((edits[0].start, edits[0].end), (0, 2));
        assert_eq!((edits[1].start, edits[1].end), (5, 7));

        let op = Operation::Insert {
            after: "ab".into(),
            insert: "-".into(),
        };
        let edits = op.edits_for(b"abab");
        assert_eq!(edits.len(), 2);
        assert_eq!((edits[0].start, edits[0].end), (2, 2));
        assert_eq!((edits[1].start, edits[1].end), (4, 4));
    }

    #[test]
    fn build_diffs_lines_and_truncation() {
        let data = b"aaaaa\n00000 hello world and beyond\nlast";
        let hello = memmem::find(b"00000 hello", b"hello").unwrap();
        let start = 6 + hello;
        let edits = vec![Edit {
            start,
            end: start + 5,
            replacement: b"hi".to_vec(),
        }];
        let diffs = build_diffs(data, &edits, 4);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].line_no, 2);
        assert!(diffs[0].start.starts_with('\u{2026}'), "应带前置省略号");
        assert!(diffs[0].start.contains("hello"));
        assert!(diffs[0].new.contains("hi"));
    }

    #[test]
    fn build_diffs_insert_after_anchor() {
        let data = "你好世界".as_bytes();
        let edits = vec![Edit {
            start: "你好".as_bytes().len(),
            end: "你好".as_bytes().len(),
            replacement: " !!".as_bytes().to_vec(),
        }];
        let diffs = build_diffs(data, &edits, 24);
        assert_eq!(diffs[0].line_no, 1);
        assert_eq!(diffs[0].start, "你好世界");
        assert_eq!(diffs[0].new, "你好 !!世界");
    }

    #[test]
    fn build_diffs_one_edit_per_line() {
        let data = b"a\nb\nc";
        let edits = vec![
            Edit { start: 2, end: 2, replacement: b"@".to_vec() },
            Edit { start: 4, end: 4, replacement: b"@".to_vec() },
        ];
        let diffs = build_diffs(data, &edits, 24);
        assert_eq!(diffs.len(), 2);
        assert_eq!(diffs[0].line_no, 2);
        assert_eq!(diffs[1].line_no, 3);
        assert_eq!(diffs[0].start, "b");
        assert_eq!(diffs[0].new, "@b");
        assert_eq!(diffs[1].start, "c");
        assert_eq!(diffs[1].new, "@c");
    }

    #[test]
    fn build_diffs_same_line_multiple_hits() {
        let data = b"ab ab";
        let edits = [
            Edit { start: 0, end: 2, replacement: b"X".to_vec() },
            Edit { start: 3, end: 5, replacement: b"Y".to_vec() },
        ];
        let diffs = build_diffs(data, &edits, 24);
        assert_eq!(diffs.len(), 2);
        assert_eq!(diffs[0].line_no, 1);
        assert_eq!(diffs[1].line_no, 1);
        assert_eq!(diffs[0].new, "X ab");
        assert_eq!(diffs[1].new, "ab Y");
    }

    #[test]
    fn canonical_clean_strips_verbatim_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("sub");
        std::fs::create_dir(&dir).unwrap();
        let out = canonical_clean(&dir);
        assert!(out.is_absolute());
        assert!(out.exists());
        assert!(!out.to_string_lossy().starts_with(r"\\?\"));
    }

    #[test]
    fn canonical_clean_falls_back_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("nope");
        let out = canonical_clean(&missing);
        assert_eq!(out, missing);
    }
}
