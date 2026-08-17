use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

use txtbatch::config;
use txtbatch::{canonical_clean, run_edit, Operation};

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
