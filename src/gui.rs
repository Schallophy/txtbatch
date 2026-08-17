use std::path::PathBuf;

use eframe::egui::{self, Color32, FontId, RichText, ScrollArea, TextFormat, Ui};
use eframe::egui::text::LayoutJob;

use crate::config;
use crate::{process_dir, Operation, Summary};

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Replace,
    Insert,
}

enum PreviewRow {
    FileHeader { path: String, count: usize },
    Diff { line_no: usize, num_width: usize, old: String, new: String },
    Separator,
}

pub struct App {
    dir: String,
    mode: Mode,
    find: String,
    replace_with: String,
    after: String,
    insert_text: String,

    preview: Option<Summary>,
    status: String,
    status_color: Color32,
}

impl Default for App {
    fn default() -> Self {
        let saved_dir = config::load_dir().ok().flatten().unwrap_or_default();
        Self {
            dir: saved_dir,
            mode: Mode::Replace,
            find: String::new(),
            replace_with: String::new(),
            after: String::new(),
            insert_text: String::new(),
            preview: None,
            status: String::new(),
            status_color: Color32::GRAY,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::bottom("bottom_bar").show(ctx, |ui| {
            if !self.status.is_empty() {
                ui.colored_label(self.status_color, &self.status);
            }
        });

        egui::SidePanel::left("controls").resizable(true).default_width(340.0).show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                self.render_controls(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_preview(ui);
        });
    }
}

impl App {
    fn render_controls(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("目录:");
            let response = ui.text_edit_singleline(&mut self.dir);
            if response.changed() {
                self.preview = None;
            }
            if response.lost_focus() {
                let _ = config::save_dir(&self.dir);
            }
            if ui.button("浏览...").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("选择工作目录")
                    .pick_folder()
                {
                    self.dir = path.to_string_lossy().to_string();
                    let _ = config::save_dir(&self.dir);
                    self.preview = None;
                }
            }
        });
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("操作模式:");
            ui.selectable_value(&mut self.mode, Mode::Replace, "替换");
            ui.selectable_value(&mut self.mode, Mode::Insert, "插入");
        });
        ui.add_space(4.0);

        match self.mode {
            Mode::Replace => {
                ui.label("查找文本:");
                ui.text_edit_multiline(&mut self.find);
                ui.add_space(4.0);
                ui.label("替换为:");
                ui.text_edit_multiline(&mut self.replace_with);
            }
            Mode::Insert => {
                ui.label("定位文本 (在其后插入):");
                ui.text_edit_multiline(&mut self.after);
                ui.add_space(4.0);
                ui.label("插入内容:");
                ui.text_edit_multiline(&mut self.insert_text);
            }
        }

        ui.add_space(12.0);
        ui.separator();

        let can_preview = !self.dir.is_empty()
            && match self.mode {
                Mode::Replace => !self.find.is_empty(),
                Mode::Insert => !self.after.is_empty(),
            };

        let can_apply = can_preview && self.preview.is_some();

        if ui.add_enabled(can_preview, egui::Button::new(
            RichText::new("预览更改").strong(),
        )).clicked() {
            self.run_preview();
        }

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui.add_enabled(can_apply, egui::Button::new(
                RichText::new("确认应用").strong().color(Color32::from_rgb(34, 139, 34)),
            )).clicked() {
                self.apply_changes(false);
            }

            if ui.add_enabled(can_apply, egui::Button::new(
                RichText::new("取消").strong(),
            )).clicked() {
                self.preview = None;
                self.status = "已取消".to_string();
                self.status_color = Color32::GRAY;
            }
        });

        if let Some(ref summary) = self.preview {
            ui.add_space(12.0);
            ui.separator();
            ui.label(RichText::new("预览统计").strong());
            ui.label(format!("扫描文件: {}", summary.files_scanned));
            ui.label(format!("将修改文件: {}", summary.files_modified));
            ui.label(format!("修改处数: {}", summary.total_edits));
            if summary.binary_skipped > 0 {
                ui.label(format!("跳过二进制: {}", summary.binary_skipped));
            }
            if !summary.errors.is_empty() {
                ui.label(RichText::new(format!("错误: {}", summary.errors.len()))
                    .color(Color32::RED));
            }
        }
    }

    fn render_preview(&self, ui: &mut Ui) {
        match &self.preview {
            Some(summary) if summary.files_modified > 0 => {
                let mono = FontId::monospace(14.0);
                let row_height = 20.0;
                let num_color = Color32::from_rgb(100, 100, 100);
                let old_color = Color32::from_rgb(220, 80, 80);
                let new_color = Color32::from_rgb(80, 200, 80);

                // Build flat row list with per-file line number width
                let mut rows: Vec<PreviewRow> = Vec::new();
                for file_edit in &summary.details {
                    let max_line = file_edit.diffs.iter().map(|d| d.line_no).max().unwrap_or(0);
                    let nw = format!("{max_line}").len().max(3);
                    rows.push(PreviewRow::FileHeader {
                        path: file_edit.path.display().to_string(),
                        count: file_edit.diffs.len(),
                    });
                    for diff in &file_edit.diffs {
                        rows.push(PreviewRow::Diff {
                            line_no: diff.line_no,
                            num_width: nw,
                            old: diff.start.clone(),
                            new: diff.new.clone(),
                        });
                    }
                    rows.push(PreviewRow::Separator);
                }

                ScrollArea::both()
                    .auto_shrink([false, false])
                    .show_rows(ui, row_height, rows.len(), |ui, row_range| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        for i in row_range {
                            match &rows[i] {
                                PreviewRow::FileHeader { path, count } => {
                                    let mut job = LayoutJob::default();
                                    job.append(
                                        &format!("── {path} ({count} 处修改) ──\n"),
                                        0.0,
                                        TextFormat {
                                            font_id: FontId::proportional(13.0),
                                            color: Color32::from_rgb(70, 130, 180),
                                            ..Default::default()
                                        },
                                    );
                                    ui.label(job);
                                }
                                PreviewRow::Diff { line_no, num_width, old, new } => {
                                    let pad = " ".repeat(*num_width);
                                    let prefix = format!("{line_no:>width$} │ ", width = num_width);
                                    let cont = format!("{pad} │ ");

                                    let mut job = LayoutJob::default();
                                    job.wrap.max_width = f32::INFINITY;
                                    // Old line
                                    job.append(&prefix, 0.0, TextFormat { font_id: mono.clone(), color: num_color, ..Default::default() });
                                    job.append(&format!("- {old}\n"), 0.0, TextFormat { font_id: mono.clone(), color: old_color, ..Default::default() });
                                    // New line
                                    job.append(&cont, 0.0, TextFormat { font_id: mono.clone(), color: num_color, ..Default::default() });
                                    job.append(&format!("+ {new}\n"), 0.0, TextFormat { font_id: mono.clone(), color: new_color, ..Default::default() });
                                    ui.add(egui::Label::new(egui::WidgetText::LayoutJob(job)).wrap_mode(egui::TextWrapMode::Extend));
                                }
                                PreviewRow::Separator => {
                                    ui.separator();
                                }
                            }
                        }
                    });
            }
            _ => {
                ui.centered_and_justified(|ui| {
                    ui.label("在左侧设置参数后，点击「预览更改」查看修改结果");
                });
            }
        }
    }

    fn make_operation(&self) -> Operation {
        match self.mode {
            Mode::Replace => Operation::Replace {
                find: self.find.clone(),
                replace: self.replace_with.clone(),
            },
            Mode::Insert => Operation::Insert {
                after: self.after.clone(),
                insert: self.insert_text.clone(),
            },
        }
    }

    fn run_preview(&mut self) {
        let dir = PathBuf::from(&self.dir);
        if !dir.is_dir() {
            self.status = format!("目录不存在: {}", self.dir);
            self.status_color = Color32::RED;
            return;
        }
        let op = self.make_operation();
        match process_dir(&dir, &op, true, usize::MAX) {
            Ok(summary) => {
                let n = summary.total_edits;
                self.status = format!("预览完成，共 {} 处修改", n);
                self.status_color = Color32::from_rgb(34, 139, 34);
                self.preview = Some(summary);
            }
            Err(e) => {
                self.status = format!("预览失败: {e}");
                self.status_color = Color32::RED;
                self.preview = None;
            }
        }
    }

    fn apply_changes(&mut self, _dry_run: bool) {
        let dir = PathBuf::from(&self.dir);
        let op = self.make_operation();
        match process_dir(&dir, &op, false, usize::MAX) {
            Ok(summary) => {
                let n = summary.total_edits;
                self.status = format!("已成功应用 {} 处修改到 {} 个文件", n, summary.files_modified);
                self.status_color = Color32::from_rgb(34, 139, 34);
                self.preview = Some(summary);
            }
            Err(e) => {
                self.status = format!("应用失败: {e}");
                self.status_color = Color32::RED;
            }
        }
    }
}
