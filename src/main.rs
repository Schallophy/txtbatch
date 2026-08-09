use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;

use txtbatch::config;
use txtbatch::{process_dir, Operation};

#[derive(Parser)]
#[command(name = "txtbatch", version, about = "对指定目录下所有文本文件进行查找替换或文本插入")]
struct Args {
    /// 本次操作的目录；省略时使用上次保存的默认目录
    #[arg(long, value_name = "目录", global = true)]
    dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,

    /// 仅预览统计，不实际写入文件
    #[arg(long, global = true)]
    dry_run: bool,

    /// 显示每一处的 diff 详情（行号与修改前后内容）
    #[arg(long, global = true)]
    diff: bool,
}

#[derive(Subcommand)]
enum Command {
    /// 在文件中查找文本并替换
    Replace {
        /// 要查找的文本
        #[arg(value_name = "查找")]
        find: String,
        /// 替换为的文本
        #[arg(value_name = "替换")]
        replace: String,
    },
    /// 在指定文本之后插入内容
    Insert {
        /// 定位文本
        #[arg(value_name = "定位")]
        after: String,
        /// 要插入的文本
        #[arg(value_name = "插入")]
        insert: String,
    },
    /// 设置默认目录（之后可省略 --dir）
    SetDir {
        /// 要设为默认的目录
        #[arg(value_name = "目录")]
        path: PathBuf,
    },
    /// 显示当前默认目录
    ShowDir,
    /// 清除默认目录
    ClearDir,
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Replace { find, replace } => {
            if find.is_empty() {
                bail!("查找文本不能为空");
            }
            run_edit(args.dir, Operation::Replace { find, replace }, args.dry_run, args.diff)
        }
        Command::Insert { after, insert } => {
            if after.is_empty() {
                bail!("定位文本不能为空");
            }
            run_edit(args.dir, Operation::Insert { after, insert }, args.dry_run, args.diff)
        }
        Command::SetDir { path } => {
            let dir = canonical_clean(&path);
            if !dir.is_dir() {
                bail!("目录不存在: {}", path.display());
            }
            let dir = dir.to_string_lossy().to_string();
            config::save_dir(&dir)?;
            println!("已设置默认目录: {dir}");
            Ok(())
        }
        Command::ShowDir => {
            match config::load_dir()? {
                Some(dir) => {
                    println!("当前默认目录: {dir}");
                    Ok(())
                }
                None => {
                    println!("尚未设置默认目录");
                    Ok(())
                }
            }
        }
        Command::ClearDir => {
            config::clear_dir()?;
            println!("已清除默认目录");
            Ok(())
        }
    }
}

fn canonical_clean(path: &Path) -> PathBuf {
    let canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = canon.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        canon
    }
}

fn run_edit(dir_arg: Option<PathBuf>, op: Operation, dry_run: bool, show_diff: bool) -> Result<()> {
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
    let summary = process_dir(&dir, &op, dry_run)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn canonical_clean_strips_verbatim_prefix() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("sub");
        fs::create_dir(&dir).unwrap();
        let out = canonical_clean(&dir);
        assert!(out.is_absolute());
        assert!(out.exists());
        assert!(!out.to_string_lossy().starts_with(r"\\?\"));
    }

    #[test]
    fn canonical_clean_falls_back_when_missing() {
        let tmp = tempdir().unwrap();
        let missing = tmp.path().join("nope");
        let out = canonical_clean(&missing);
        assert_eq!(out, missing);
    }
}







