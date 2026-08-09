use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ini::Ini;

const FILE_NAME: &str = "config.ini";
const SECTION: &str = "default";
const KEY: &str = "dir";

fn config_dir() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        let base = env::var_os("APPDATA")
            .or_else(|| env::var_os("USERPROFILE"))
            .or_else(|| env::var_os("HOME"))
            .context("无法定位配置目录（缺少 APPDATA/USERPROFILE/HOME）")?;
        Ok(PathBuf::from(base).join("txtbatch"))
    }
    #[cfg(not(windows))]
    {
        let base = if let Some(dir) = env::var_os("XDG_CONFIG_HOME") {
            PathBuf::from(dir)
        } else if let Some(home) = env::var_os("HOME") {
            PathBuf::from(home).join(".config")
        } else {
            anyhow::bail!("无法定位配置目录（缺少 XDG_CONFIG_HOME/HOME）");
        };
        Ok(base.join("txtbatch"))
    }
}

pub fn config_file() -> Result<PathBuf> {
    Ok(config_dir()?.join(FILE_NAME))
}

pub fn load_dir() -> Result<Option<String>> {
    let file = config_file()?;
    load_from(&file)
}

pub fn save_dir(dir: &str) -> Result<()> {
    let file = config_file()?;
    save_to(&file, dir)
}

pub fn clear_dir() -> Result<()> {
    let file = config_file()?;
    clear_at(&file)
}

fn load_from(file: &Path) -> Result<Option<String>> {
    if !file.exists() {
        return Ok(None);
    }
    let conf = Ini::load_from_file(file)
        .with_context(|| format!("读取配置失败: {}", file.display()))?;
    let value = conf
        .get_from(Some(SECTION), KEY)
        .map(|s| s.trim().trim_matches('\0').trim().to_string());
    Ok(value.filter(|s| !s.is_empty()))
}

fn save_to(file: &Path, dir: &str) -> Result<()> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("创建配置目录失败: {}", parent.display()))?;
    }
    let mut conf = Ini::new();
    conf.with_section(Some(SECTION)).set(KEY, dir);
    conf.write_to_file(file)
        .with_context(|| format!("写入配置失败: {}", file.display()))
}

fn clear_at(file: &Path) -> Result<()> {
    if file.exists() {
        fs::remove_file(file).with_context(|| format!("删除配置失败: {}", file.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn config_path(tmp: &Path) -> PathBuf {
        tmp.join("config")
    }

    #[test]
    fn save_load_roundtrip() {
        let tmp = tempdir().unwrap();
        let file = config_path(tmp.path());
        save_to(&file, r"C:\dir\somewhere").unwrap();
        assert_eq!(load_from(&file).unwrap(), Some(r"C:\dir\somewhere".to_string()));
    }

    #[test]
    fn save_creates_parent_dirs() {
        let tmp = tempdir().unwrap();
        let file = tmp.path().join("a/b/c/config");
        save_to(&file, "D:\\x").unwrap();
        assert!(file.exists());
    }

    #[test]
    fn missing_file_loads_none() {
        let tmp = tempdir().unwrap();
        let file = config_path(tmp.path());
        assert_eq!(load_from(&file).unwrap(), None);
    }

    #[test]
    fn blank_value_loads_none() {
        let tmp = tempdir().unwrap();
        let file = config_path(tmp.path());
        fs::write(&file, "[default]\ndir=\n").unwrap();
        assert_eq!(load_from(&file).unwrap(), None);
    }

    #[test]
    fn clear_removes_file() {
        let tmp = tempdir().unwrap();
        let file = config_path(tmp.path());
        save_to(&file, "C:\\x").unwrap();
        clear_at(&file).unwrap();
        assert!(!file.exists());
        assert_eq!(load_from(&file).unwrap(), None);
    }

    #[test]
    fn clear_absent_file_ok() {
        let tmp = tempdir().unwrap();
        let file = config_path(tmp.path());
        clear_at(&file).unwrap();
        assert!(!file.exists());
    }
}