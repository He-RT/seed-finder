use std::{
    collections::BTreeSet,
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use eframe::egui::{
    self, Button, Color32, Context as EguiContext, CornerRadius, FontData, FontDefinitions,
    FontFamily, Frame, Margin, RichText, ScrollArea, Stroke, TextureHandle, TextureOptions, Vec2,
};

use crate::{
    analysis::{format_coordinate_copy, format_nether_info, format_teleport_command},
    config::{Controls, VERSION_ENTRIES, search_worker_count, total_seeds},
    log_diag,
    preview::generate_preview,
    search::run_search,
    templates::{SearchTemplate, get_templates_by_category},
    types::{
        BIOME_FILTER_OPTIONS, FilterOption, MatchSummary, PreviewRequest, PreviewResponse,
        STRUCTURE_FILTER_OPTIONS, WorkerMessage,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Copper,
    Coast,
    Moss,
}

impl Theme {
    fn label(self) -> &'static str {
        match self {
            Theme::Copper => "铜版",
            Theme::Coast => "海图",
            Theme::Moss => "林野",
        }
    }

    fn colors(self) -> Palette {
        match self {
            Theme::Copper => Palette {
                bg: Color32::from_rgb(14, 16, 20),
                panel: Color32::from_rgb(24, 27, 34),
                panel_alt: Color32::from_rgb(31, 35, 43),
                panel_soft: Color32::from_rgb(42, 36, 32),
                text: Color32::from_rgb(238, 232, 223),
                text_muted: Color32::from_rgb(161, 154, 147),
                accent: Color32::from_rgb(220, 153, 90),
                accent_soft: Color32::from_rgb(123, 88, 58),
                border: Color32::from_rgb(77, 73, 68),
                success: Color32::from_rgb(110, 199, 134),
                warning: Color32::from_rgb(231, 191, 101),
                danger: Color32::from_rgb(233, 120, 108),
                preview_bg: Color32::from_rgb(10, 12, 15),
            },
            Theme::Coast => Palette {
                bg: Color32::from_rgb(8, 17, 24),
                panel: Color32::from_rgb(14, 27, 38),
                panel_alt: Color32::from_rgb(19, 35, 47),
                panel_soft: Color32::from_rgb(21, 46, 58),
                text: Color32::from_rgb(228, 243, 247),
                text_muted: Color32::from_rgb(132, 171, 182),
                accent: Color32::from_rgb(102, 198, 206),
                accent_soft: Color32::from_rgb(50, 104, 112),
                border: Color32::from_rgb(49, 87, 101),
                success: Color32::from_rgb(109, 208, 160),
                warning: Color32::from_rgb(226, 190, 108),
                danger: Color32::from_rgb(227, 122, 119),
                preview_bg: Color32::from_rgb(6, 13, 18),
            },
            Theme::Moss => Palette {
                bg: Color32::from_rgb(12, 18, 14),
                panel: Color32::from_rgb(21, 29, 22),
                panel_alt: Color32::from_rgb(28, 39, 29),
                panel_soft: Color32::from_rgb(34, 47, 29),
                text: Color32::from_rgb(236, 244, 229),
                text_muted: Color32::from_rgb(146, 170, 141),
                accent: Color32::from_rgb(154, 198, 98),
                accent_soft: Color32::from_rgb(76, 109, 54),
                border: Color32::from_rgb(60, 82, 54),
                success: Color32::from_rgb(112, 207, 128),
                warning: Color32::from_rgb(224, 193, 94),
                danger: Color32::from_rgb(224, 124, 102),
                preview_bg: Color32::from_rgb(8, 12, 9),
            },
        }
    }
}

#[derive(Clone, Copy)]
struct Palette {
    bg: Color32,
    panel: Color32,
    panel_alt: Color32,
    panel_soft: Color32,
    text: Color32,
    text_muted: Color32,
    accent: Color32,
    accent_soft: Color32,
    border: Color32,
    success: Color32,
    warning: Color32,
    danger: Color32,
    preview_bg: Color32,
}

pub struct SeedFinderApp {
    pub controls: Controls,
    pub results: Vec<MatchSummary>,
    pub selected_result: Option<usize>,
    pub search_rx: Option<std::sync::mpsc::Receiver<WorkerMessage>>,
    pub preview_rx: Option<std::sync::mpsc::Receiver<PreviewResponse>>,
    pub search_cancel: Option<Arc<AtomicBool>>,
    pub preview_token: u64,
    pub preview_texture: Option<TextureHandle>,
    pub preview_status: String,
    pub status_line: String,
    pub info_line: String,
    pub progress: Option<(usize, usize, usize, i64)>,
    pub is_searching: bool,
    pub last_error: Option<String>,
    pub theme: Theme,
    pub applied_theme: Option<Theme>,
    pub favorites: Vec<MatchSummary>,
    pub selected_template: Option<String>,
    pub show_template_panel: bool,
}

impl SeedFinderApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_fonts(&cc.egui_ctx);
        configure_theme(&cc.egui_ctx, Theme::Copper.colors());

        Self {
            controls: Controls::default(),
            results: Vec::new(),
            selected_result: None,
            search_rx: None,
            preview_rx: None,
            search_cancel: None,
            preview_token: 0,
            preview_texture: None,
            preview_status: "选择结果后生成地图预览".into(),
            status_line: "准备就绪".into(),
            info_line: "先选模板，或直接调整条件后开始筛种。".into(),
            progress: None,
            is_searching: false,
            last_error: None,
            theme: Theme::Copper,
            applied_theme: None,
            favorites: Vec::new(),
            selected_template: None,
            show_template_panel: true,
        }
    }

    pub fn start_search(&mut self, ctx: &EguiContext) {
        log_diag("ui_start_search_clicked");
        let config = match self.controls.to_config() {
            Ok(config) => config,
            Err(err) => {
                self.last_error = Some(err.to_string());
                self.status_line = "参数无效".into();
                return;
            }
        };

        self.stop_search();
        self.results.clear();
        self.selected_result = None;
        self.preview_rx = None;
        self.preview_texture = None;
        self.preview_status = "正在等待搜索结果，点击命中项后会在这里显示地图。".into();
        self.progress = Some((0, total_seeds(&config), 0, config.seed_start));
        self.status_line = "正在搜索".into();
        let worker_count = search_worker_count(&config);
        self.info_line = config.version_warning.clone().unwrap_or_else(|| {
            format!(
                "范围 {}..={}，版本 {}，并行线程 {}",
                config.seed_start, config.seed_end, config.version_label, worker_count
            )
        });
        self.last_error = None;

        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = Arc::clone(&cancel);
        let ctx = ctx.clone();

        thread::spawn(move || {
            let result = run_search(config, cancel_clone, tx.clone());
            if let Err(err) = result {
                let _ = tx.send(WorkerMessage::Error(err.to_string()));
            }
            ctx.request_repaint();
        });

        self.search_rx = Some(rx);
        self.search_cancel = Some(cancel);
        self.is_searching = true;
    }

    pub fn stop_search(&mut self) {
        if let Some(cancel) = &self.search_cancel {
            cancel.store(true, Ordering::Relaxed);
        }
        self.search_cancel = None;
        self.search_rx = None;
        self.is_searching = false;
    }

    pub fn apply_template(&mut self, template: &SearchTemplate) {
        self.controls.seed_start = template.config.seed_start.clone();
        self.controls.seed_end = template.config.seed_end.clone();
        self.controls.version_index = template.config.version_index;
        self.controls.limit = template.config.limit.clone();
        self.controls.require_biome = template.config.require_biome.clone();
        self.controls.forbid_biome = template.config.forbid_biome.clone();
        self.controls.biome_radius = template.config.biome_radius.clone();
        self.controls.biome_step = template.config.biome_step.clone();
        self.controls.require_structure = template.config.require_structure.clone();
        self.controls.structure_radius = template.config.structure_radius.clone();
        self.controls.terrain_radius = template.config.terrain_radius.clone();
        self.controls.min_avg_height = template.config.min_avg_height.clone();
        self.controls.max_avg_height = template.config.max_avg_height.clone();
        self.controls.max_relief = template.config.max_relief.clone();
        self.selected_template = Some(template.id.to_string());
        self.status_line = format!("已加载模板: {}", template.name);
        self.info_line = template.description.into();
    }

    pub fn clear_results(&mut self) {
        self.stop_search();
        self.results.clear();
        self.selected_result = None;
        self.preview_rx = None;
        self.preview_texture = None;
        self.preview_status = "选择结果后生成地图预览".into();
        self.status_line = "结果已清空".into();
        self.info_line = "可以继续调整条件后重新搜索。".into();
        self.progress = None;
        self.last_error = None;
    }

    pub fn toggle_favorite(&mut self, index: usize) {
        if index >= self.results.len() {
            return;
        }
        self.results[index].is_favorite = !self.results[index].is_favorite;
        if self.results[index].is_favorite {
            self.favorites.push(self.results[index].clone());
        } else {
            self.favorites
                .retain(|f| f.seed != self.results[index].seed);
        }
    }

    pub fn export_results(&self) -> String {
        let mut output = String::new();
        output.push_str("# Minecraft Seed Finder Export\n");
        output.push_str(&format!("# Total seeds: {}\n\n", self.results.len()));

        for summary in &self.results {
            output.push_str(&format!("Seed: {}\n", summary.seed));
            output.push_str(&format!("  Version: {}\n", summary.version_label));
            output.push_str(&format!(
                "  Spawn: ({}, {})\n",
                summary.spawn.x, summary.spawn.z
            ));
            if let Some(score) = &summary.score {
                output.push_str(&format!(
                    "  Score: {} ({:.1})\n",
                    score.grade.display(),
                    score.overall
                ));
            }
            output.push('\n');
        }
        output
    }

    fn receive_worker_messages(&mut self) {
        let mut clear = false;
        if let Some(rx) = &self.search_rx {
            while let Ok(message) = rx.try_recv() {
                match message {
                    WorkerMessage::Progress {
                        searched,
                        total,
                        found,
                        current_seed,
                    } => {
                        self.progress = Some((searched, total, found, current_seed));
                        self.status_line = format!("正在搜索，已检查 {searched}/{total}");
                        self.info_line = format!("当前种子 {current_seed}，命中 {found}");
                    }
                    WorkerMessage::Match(summary) => {
                        self.results.push(summary);
                        self.status_line = format!("已找到 {} 个候选种子", self.results.len());
                        self.info_line = "点击右侧结果卡片查看地图与分析。".into();
                    }
                    WorkerMessage::Finished {
                        searched,
                        found,
                        stopped,
                    } => {
                        self.is_searching = false;
                        self.search_cancel = None;
                        self.progress = None;
                        self.status_line = if stopped {
                            format!("搜索已停止，累计检查 {searched} 个种子")
                        } else {
                            format!("搜索完成，共命中 {found} 个种子")
                        };
                        self.info_line = if found == 0 {
                            "没有找到符合条件的结果。".into()
                        } else {
                            "结果已更新，点击查看详情。".into()
                        };
                        clear = true;
                    }
                    WorkerMessage::Error(err) => {
                        self.is_searching = false;
                        self.search_cancel = None;
                        self.progress = None;
                        self.last_error = Some(err);
                        self.status_line = "搜索失败".into();
                        clear = true;
                    }
                }
            }
        }
        if clear {
            self.search_rx = None;
        }
    }

    fn receive_preview_messages(&mut self, ctx: &EguiContext) {
        if let Some(rx) = &self.preview_rx {
            while let Ok(message) = rx.try_recv() {
                if message.token != self.preview_token {
                    continue;
                }
                match message.image {
                    Ok(image) => {
                        self.preview_texture =
                            Some(ctx.load_texture("preview-map", image, TextureOptions::LINEAR));
                        self.preview_status = "地图预览已更新".into();
                    }
                    Err(err) => {
                        self.preview_texture = None;
                        self.preview_status = err;
                    }
                }
            }
        }
    }

    pub fn select_result(&mut self, index: usize, ctx: &EguiContext) {
        if index >= self.results.len() {
            return;
        }
        self.selected_result = Some(index);
        self.preview_status = format!("正在加载种子 {} 的地图预览", self.results[index].seed);
        self.start_preview(self.results[index].clone(), ctx);
    }

    pub fn start_preview(&mut self, summary: MatchSummary, ctx: &EguiContext) {
        let radius = match self.controls.preview_radius() {
            Ok(radius) => radius,
            Err(err) => {
                self.preview_texture = None;
                self.preview_status = err.to_string();
                return;
            }
        };
        let image_size = match self.controls.preview_image_size() {
            Ok(size) => size,
            Err(err) => {
                self.preview_texture = None;
                self.preview_status = err.to_string();
                return;
            }
        };

        self.preview_token = self.preview_token.wrapping_add(1);
        self.preview_texture = None;

        let request = PreviewRequest {
            token: self.preview_token,
            summary,
            radius,
            image_size,
        };

        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        thread::spawn(move || {
            let image = generate_preview(&request).map_err(|err| err.to_string());
            let _ = tx.send(PreviewResponse {
                token: request.token,
                image,
            });
            ctx.request_repaint();
        });
        self.preview_rx = Some(rx);
    }

    pub fn draw_ui(&mut self, ctx: &EguiContext) {
        let colors = self.theme.colors();
        if self.applied_theme != Some(self.theme) {
            configure_theme(ctx, colors);
            self.applied_theme = Some(self.theme);
        }

        egui::TopBottomPanel::top("topbar")
            .frame(
                Frame::default()
                    .fill(colors.panel_alt)
                    .inner_margin(Margin::symmetric(20, 12)),
            )
            .show(ctx, |ui| self.draw_topbar(ui, colors));

        egui::SidePanel::left("left_controls")
            .resizable(true)
            .default_width(340.0)
            .min_width(280.0)
            .frame(
                Frame::default()
                    .fill(colors.panel)
                    .inner_margin(Margin::same(16)),
            )
            .show(ctx, |ui| self.draw_left_panel(ui, ctx, colors));

        if self.selected_result.is_some() || self.is_searching {
            egui::SidePanel::right("right_inspector")
                .resizable(true)
                .default_width(420.0)
                .min_width(360.0)
                .frame(
                    Frame::default()
                        .fill(colors.panel)
                        .inner_margin(Margin::same(16)),
                )
                .show(ctx, |ui| self.draw_right_inspector(ui, colors));
        }

        egui::CentralPanel::default()
            .frame(
                Frame::default()
                    .fill(colors.bg)
                    .inner_margin(Margin::same(20)),
            )
            .show(ctx, |ui| self.draw_center_ledger(ui, ctx, colors));
    }

    fn draw_topbar(&mut self, ui: &mut egui::Ui, colors: Palette) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Seed Finder")
                            .size(20.0)
                            .color(colors.text)
                            .strong()
                            .monospace(),
                    );
                    ui.label(
                        RichText::new("数据分析控制台")
                            .size(12.0)
                            .color(colors.accent)
                            .monospace(),
                    );
                });
                ui.label(
                    RichText::new(&self.info_line)
                        .size(12.0)
                        .color(colors.text_muted),
                );
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for theme in [Theme::Copper, Theme::Coast, Theme::Moss] {
                    let is_selected = self.theme == theme;
                    let text_color = if is_selected {
                        colors.bg
                    } else {
                        colors.text_muted
                    };
                    let fill_color = if is_selected {
                        colors.accent
                    } else {
                        colors.panel_soft
                    };
                    let mut button =
                        Button::new(RichText::new(theme.label()).size(12.0).color(text_color));
                    if is_selected {
                        button = button.fill(fill_color);
                    }
                    if ui.add(button).clicked() {
                        self.theme = theme;
                    }
                }

                ui.add_space(16.0);
                stat_badge(ui, colors, "收藏", &self.favorites.len().to_string());
                stat_badge(ui, colors, "状态", &self.status_line);
            });
        });
    }

    fn draw_left_panel(&mut self, ui: &mut egui::Ui, ctx: &EguiContext, colors: Palette) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(4.0);

            // Templates Section
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("模板库")
                        .size(13.0)
                        .monospace()
                        .color(colors.text_muted)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button(if self.show_template_panel {
                            "收起"
                        } else {
                            "展开"
                        })
                        .clicked()
                    {
                        self.show_template_panel = !self.show_template_panel;
                    }
                });
            });
            ui.add_space(8.0);
            if self.show_template_panel {
                self.draw_template_strip(ui, colors);
                ui.add_space(16.0);
            }

            // Parameters Section
            ui.label(
                RichText::new("搜索参数")
                    .size(13.0)
                    .monospace()
                    .color(colors.text_muted)
                    .strong(),
            );
            ui.add_space(8.0);
            Frame::default()
                .fill(colors.panel_alt)
                .stroke(Stroke::new(1.0, colors.border))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    self.draw_controls_form(ui, colors);
                });

            ui.add_space(16.0);

            // Action Row
            self.draw_action_row(ui, ctx, colors);

            ui.add_space(20.0);

            // Progress Section
            ui.label(
                RichText::new("任务监控")
                    .size(13.0)
                    .monospace()
                    .color(colors.text_muted)
                    .strong(),
            );
            ui.add_space(8.0);
            Frame::default()
                .fill(colors.panel_alt)
                .stroke(Stroke::new(1.0, colors.border))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    if let Some((searched, total, found, current_seed)) = self.progress {
                        let progress = if total == 0 {
                            0.0
                        } else {
                            searched as f32 / total as f32
                        };
                        ui.add(
                            egui::ProgressBar::new(progress)
                                .desired_width(ui.available_width())
                                .fill(colors.accent)
                                .text(format!("{} / {}", searched, total)),
                        );
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("当前种子")
                                        .size(10.0)
                                        .color(colors.text_muted),
                                );
                                ui.label(
                                    RichText::new(current_seed.to_string())
                                        .size(12.0)
                                        .monospace()
                                        .color(colors.text),
                                );
                            });
                            ui.add_space(20.0);
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new("命中数").size(10.0).color(colors.text_muted),
                                );
                                ui.label(
                                    RichText::new(found.to_string())
                                        .size(12.0)
                                        .monospace()
                                        .color(colors.success),
                                );
                            });
                        });
                    } else {
                        ui.label(
                            RichText::new("空闲中")
                                .size(12.0)
                                .monospace()
                                .color(colors.text_muted),
                        );
                    }
                    if let Some(error) = &self.last_error {
                        ui.add_space(8.0);
                        warn_box(ui, colors.danger, error);
                    }
                });
        });
    }

    fn draw_template_strip(&mut self, ui: &mut egui::Ui, colors: Palette) {
        for (category, templates) in get_templates_by_category() {
            ui.label(
                RichText::new(format!("{} {}", category.icon(), category.display()))
                    .size(12.0)
                    .color(colors.text_muted),
            );
            ui.add_space(4.0);
            for template in templates {
                let selected = self.selected_template.as_deref() == Some(template.id);
                let fill = if selected {
                    colors.panel_soft
                } else {
                    colors.panel_alt
                };
                let stroke_color = if selected {
                    colors.accent
                } else {
                    colors.border
                };

                let response = ui.add(
                    Button::new(RichText::new(template.name).size(13.0).color(colors.text))
                        .fill(fill)
                        .stroke(Stroke::new(1.0, stroke_color))
                        .min_size(Vec2::new(ui.available_width(), 32.0)),
                );

                if response.clicked() {
                    self.apply_template(&template);
                }
                response.on_hover_text(template.description);
                ui.add_space(4.0);
            }
            ui.add_space(8.0);
        }
    }

    fn draw_controls_form(&mut self, ui: &mut egui::Ui, colors: Palette) {
        pair_inputs(
            ui,
            colors,
            "起始种子",
            &mut self.controls.seed_start,
            "结束种子",
            &mut self.controls.seed_end,
        );

        ui.columns(2, |cols| {
            cols[0].label(
                RichText::new("游戏版本")
                    .size(11.0)
                    .color(colors.text_muted),
            );
            version_dropdown(&mut cols[0], &mut self.controls.version_index, colors);
            cols[1].label(
                RichText::new("最大命中")
                    .size(11.0)
                    .color(colors.text_muted),
            );
            text_field(&mut cols[1], colors, &mut self.controls.limit);
        });
        ui.add_space(8.0);

        input_block(ui, colors, "需要生物群系", |ui| {
            multiselect_dropdown(
                ui,
                &mut self.controls.require_biome,
                BIOME_FILTER_OPTIONS,
                "required-biome",
                colors,
            )
        });
        input_block(ui, colors, "排除生物群系", |ui| {
            multiselect_dropdown(
                ui,
                &mut self.controls.forbid_biome,
                BIOME_FILTER_OPTIONS,
                "forbidden-biome",
                colors,
            )
        });

        pair_inputs(
            ui,
            colors,
            "群系检索半径",
            &mut self.controls.biome_radius,
            "群系采样步长",
            &mut self.controls.biome_step,
        );

        input_block(ui, colors, "需要关键结构", |ui| {
            multiselect_dropdown(
                ui,
                &mut self.controls.require_structure,
                STRUCTURE_FILTER_OPTIONS,
                "required-structure",
                colors,
            )
        });

        pair_inputs(
            ui,
            colors,
            "结构检索半径",
            &mut self.controls.structure_radius,
            "地形检索半径",
            &mut self.controls.terrain_radius,
        );
        pair_inputs(
            ui,
            colors,
            "最小平均高",
            &mut self.controls.min_avg_height,
            "最大平均高",
            &mut self.controls.max_avg_height,
        );
        pair_inputs(
            ui,
            colors,
            "最大高差",
            &mut self.controls.max_relief,
            "预览图半径",
            &mut self.controls.preview_radius,
        );

        input_block(ui, colors, "预览图尺寸", |ui| {
            text_field(ui, colors, &mut self.controls.preview_image_size)
        });
    }

    fn draw_action_row(&mut self, ui: &mut egui::Ui, ctx: &EguiContext, colors: Palette) {
        ui.horizontal(|ui| {
            let search_btn = Button::new(
                RichText::new(if self.is_searching {
                    "搜索中..."
                } else {
                    "开始筛种"
                })
                .size(13.0)
                .strong()
                .monospace()
                .color(colors.bg),
            )
            .fill(colors.accent)
            .min_size(Vec2::new(120.0, 36.0));

            if ui.add(search_btn).clicked() && !self.is_searching {
                self.start_search(ctx);
            }

            if ui
                .add_enabled(
                    self.is_searching,
                    Button::new(RichText::new("停止").size(12.0)).min_size(Vec2::new(60.0, 36.0)),
                )
                .clicked()
            {
                self.stop_search();
                self.status_line = "Search stopped".into();
            }

            if ui
                .add_sized([60.0, 36.0], Button::new(RichText::new("清空").size(12.0)))
                .clicked()
            {
                self.clear_results();
            }

            if ui
                .add_enabled(
                    !self.results.is_empty(),
                    Button::new(RichText::new("导出").size(12.0)).min_size(Vec2::new(60.0, 36.0)),
                )
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Text", &["txt"])
                    .save_file()
                {
                    let _ = fs::write(path, self.export_results());
                }
            }
        });
    }

    fn draw_center_ledger(&mut self, ui: &mut egui::Ui, ctx: &EguiContext, colors: Palette) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("命中数据台")
                    .size(16.0)
                    .monospace()
                    .color(colors.text)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{} MATCHES", self.results.len()))
                        .size(12.0)
                        .monospace()
                        .color(colors.text_muted),
                );
            });
        });
        ui.add_space(12.0);

        if self.results.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    RichText::new(
                        "Awaiting telemetry...
Adjust parameters and initiate search.",
                    )
                    .size(14.0)
                    .color(colors.text_muted)
                    .monospace(),
                );
            });
            return;
        }

        let mut clicked_index = None;
        let mut toggle_fav_idx = None;

        // Table header
        Frame::default()
            .fill(colors.panel_alt)
            .corner_radius(CornerRadius::same(4))
            .inner_margin(Margin::symmetric(16, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(
                        Vec2::new(180.0, 20.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.label(
                                RichText::new("种子号")
                                    .size(11.0)
                                    .monospace()
                                    .color(colors.text_muted),
                            );
                        },
                    );
                    ui.allocate_ui_with_layout(
                        Vec2::new(100.0, 20.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.label(
                                RichText::new("出生点")
                                    .size(11.0)
                                    .monospace()
                                    .color(colors.text_muted),
                            );
                        },
                    );
                    ui.allocate_ui_with_layout(
                        Vec2::new(80.0, 20.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.label(
                                RichText::new("评级")
                                    .size(11.0)
                                    .monospace()
                                    .color(colors.text_muted),
                            );
                        },
                    );
                    ui.label(
                        RichText::new("关键指标")
                            .size(11.0)
                            .monospace()
                            .color(colors.text_muted),
                    );
                });
            });

        ui.add_space(4.0);

        ScrollArea::vertical().show(ui, |ui| {
            for (index, summary) in self.results.iter().enumerate() {
                let selected = self.selected_result == Some(index);
                let bg_color = if selected {
                    colors.panel_soft
                } else {
                    colors.panel
                };

                let response = Frame::default()
                    .fill(bg_color)
                    .stroke(Stroke::new(
                        1.0,
                        if selected {
                            colors.border
                        } else {
                            Color32::TRANSPARENT
                        },
                    ))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // SEED ID
                            ui.allocate_ui_with_layout(
                                Vec2::new(180.0, 24.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(format!("{}", summary.seed))
                                            .size(15.0)
                                            .strong()
                                            .monospace()
                                            .color(colors.accent),
                                    );
                                },
                            );

                            // SPAWN
                            ui.allocate_ui_with_layout(
                                Vec2::new(100.0, 24.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "{}, {}",
                                            summary.spawn.x, summary.spawn.z
                                        ))
                                        .size(12.0)
                                        .monospace()
                                        .color(colors.text_muted),
                                    );
                                },
                            );

                            // RATING
                            ui.allocate_ui_with_layout(
                                Vec2::new(80.0, 24.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    if let Some(score) = &summary.score {
                                        let color = match score.grade {
                                            crate::scoring::SeedGrade::S => colors.warning,
                                            crate::scoring::SeedGrade::A => colors.success,
                                            crate::scoring::SeedGrade::B => colors.accent,
                                            crate::scoring::SeedGrade::C => colors.text,
                                            crate::scoring::SeedGrade::D => colors.text_muted,
                                            crate::scoring::SeedGrade::F => colors.danger,
                                        };
                                        ui.label(
                                            RichText::new(score.grade.display())
                                                .size(14.0)
                                                .strong()
                                                .color(color),
                                        );
                                    } else {
                                        ui.label("-");
                                    }
                                },
                            );

                            // METRICS
                            ui.vertical(|ui| {
                                if !summary.structures.is_empty() {
                                    let line = summary
                                        .structures
                                        .iter()
                                        .take(3)
                                        .map(|(kind, hit)| format!("{}({})", kind, hit.distance))
                                        .collect::<Vec<_>>()
                                        .join(" ");
                                    ui.label(
                                        RichText::new(line).size(11.0).color(colors.text_muted),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new("无结构数据")
                                            .size(11.0)
                                            .color(colors.text_muted)
                                            .monospace(),
                                    );
                                }
                            });

                            // FAVORITE
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let star = if summary.is_favorite { "★" } else { "☆" };
                                    let star_color = if summary.is_favorite {
                                        colors.warning
                                    } else {
                                        colors.border
                                    };
                                    if ui
                                        .add(
                                            Button::new(
                                                RichText::new(star).size(16.0).color(star_color),
                                            )
                                            .fill(Color32::TRANSPARENT)
                                            .stroke(Stroke::NONE),
                                        )
                                        .clicked()
                                    {
                                        toggle_fav_idx = Some(index);
                                    }
                                },
                            );
                        });
                    });

                if ui
                    .interact(
                        response.response.rect,
                        ui.id().with(("row", index)),
                        egui::Sense::click(),
                    )
                    .clicked()
                {
                    clicked_index = Some(index);
                }
                ui.add_space(4.0);
            }
        });

        if let Some(idx) = toggle_fav_idx {
            self.toggle_favorite(idx);
        } else if let Some(index) = clicked_index {
            self.select_result(index, ctx);
        }
    }

    fn draw_right_inspector(&mut self, ui: &mut egui::Ui, colors: Palette) {
        ScrollArea::vertical().show(ui, |ui| {
            ui.label(
                RichText::new("预览检视器")
                    .size(16.0)
                    .monospace()
                    .color(colors.text)
                    .strong(),
            );
            ui.add_space(12.0);

            // Map Preview
            Frame::default()
                .fill(colors.preview_bg)
                .stroke(Stroke::new(1.0, colors.border))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::same(8))
                .show(ui, |ui| {
                    ui.set_min_height(340.0);
                    ui.centered_and_justified(|ui| {
                        if let Some(texture) = &self.preview_texture {
                            let size = texture.size_vec2();
                            let max = ui.available_size();
                            let scale = (max.x / size.x).min(max.y / size.y).max(0.1);
                            ui.image((texture.id(), size * scale));
                        } else {
                            ui.label(
                                RichText::new(&self.preview_status)
                                    .size(12.0)
                                    .monospace()
                                    .color(colors.text_muted),
                            );
                        }
                    });
                });

            ui.add_space(16.0);

            if let Some(index) = self.selected_result {
                let summary = &self.results[index];
                self.draw_dossier(ui, summary, colors);
            } else {
                warn_box(ui, colors.warning, "请从数据台中选择一个种子以查看详细分析");
            }
        });
    }

    fn draw_dossier(&self, ui: &mut egui::Ui, summary: &MatchSummary, colors: Palette) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("SEED {}", summary.seed))
                    .size(22.0)
                    .monospace()
                    .color(colors.text)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(score) = &summary.score {
                    let color = match score.grade {
                        crate::scoring::SeedGrade::S => colors.warning,
                        crate::scoring::SeedGrade::A => colors.success,
                        crate::scoring::SeedGrade::B => colors.accent,
                        crate::scoring::SeedGrade::C => colors.text,
                        crate::scoring::SeedGrade::D => colors.text_muted,
                        crate::scoring::SeedGrade::F => colors.danger,
                    };
                    Frame::default()
                        .fill(color.gamma_multiply(0.2))
                        .stroke(Stroke::new(1.0, color))
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(Margin::symmetric(12, 4))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("RATING {}", score.grade.display()))
                                    .size(14.0)
                                    .strong()
                                    .color(color),
                            );
                        });
                }
            });
        });

        ui.add_space(12.0);

        // Key Info Grid
        ui.columns(2, |cols| {
            cols[0].vertical(|ui| {
                ui.label(
                    RichText::new("出生点坐标")
                        .size(10.0)
                        .color(colors.text_muted),
                );
                ui.label(
                    RichText::new(format!("{}, {}", summary.spawn.x, summary.spawn.z))
                        .size(14.0)
                        .monospace()
                        .color(colors.text),
                );
            });
            cols[1].vertical(|ui| {
                ui.label(
                    RichText::new("适用版本")
                        .size(10.0)
                        .color(colors.text_muted),
                );
                ui.label(
                    RichText::new(&summary.version_label)
                        .size(14.0)
                        .monospace()
                        .color(colors.text),
                );
            });
        });

        ui.add_space(16.0);

        if let Some(score) = &summary.score {
            ui.label(
                RichText::new("评分雷达")
                    .size(12.0)
                    .monospace()
                    .color(colors.text_muted),
            );
            ui.add_space(8.0);

            Frame::default()
                .fill(colors.panel_alt)
                .stroke(Stroke::new(1.0, colors.border))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        score_metric(
                            ui,
                            colors,
                            "综合",
                            format!("{:.1}", score.overall),
                            colors.accent,
                        );
                        ui.add_space(16.0);
                        score_metric(
                            ui,
                            colors,
                            "资源",
                            format!("{:.0}", score.resource_score),
                            colors.text,
                        );
                        ui.add_space(16.0);
                        score_metric(
                            ui,
                            colors,
                            "安全",
                            format!("{:.0}", score.safety_score),
                            colors.text,
                        );
                        ui.add_space(16.0);
                        score_metric(
                            ui,
                            colors,
                            "群系",
                            format!("{:.0}", score.biome_diversity),
                            colors.text,
                        );
                        ui.add_space(16.0);
                        score_metric(
                            ui,
                            colors,
                            "结构",
                            format!("{:.0}", score.structure_score),
                            colors.text,
                        );
                    });
                });
            ui.add_space(16.0);
        }

        if let Some(spawn) = &summary.spawn_analysis {
            let safety = if spawn.is_safe {
                "安全出生"
            } else {
                "高风险出生"
            };
            let safety_color = if spawn.is_safe {
                colors.success
            } else {
                colors.danger
            };
            warn_box(ui, safety_color, safety);
            ui.add_space(12.0);
        }

        ui.label(
            RichText::new("快捷指令与数据")
                .size(12.0)
                .monospace()
                .color(colors.text_muted),
        );
        ui.add_space(8.0);
        Frame::default()
            .fill(colors.panel_alt)
            .stroke(Stroke::new(1.0, colors.border))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let cp_txt = format_coordinate_copy(summary.spawn);
                    if ui.button("复制坐标").on_hover_text(&cp_txt).clicked() {
                        ui.ctx().copy_text(cp_txt.clone());
                    }

                    let tp_txt = format_teleport_command(summary.spawn, "overworld");
                    if ui.button("复制TP指令").on_hover_text(&tp_txt).clicked() {
                        ui.ctx().copy_text(tp_txt.clone());
                    }
                });
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format_nether_info(summary.spawn))
                        .size(11.0)
                        .monospace()
                        .color(colors.text_muted),
                );
            });
    }
}

impl eframe::App for SeedFinderApp {
    fn update(&mut self, ctx: &EguiContext, _frame: &mut eframe::Frame) {
        self.receive_worker_messages();
        self.receive_preview_messages(ctx);
        if self.is_searching {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        self.draw_ui(ctx);
    }
}

fn score_metric(
    ui: &mut egui::Ui,
    colors: Palette,
    label: &str,
    value: String,
    value_color: Color32,
) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).size(10.0).color(colors.text_muted));
        ui.label(
            RichText::new(value)
                .size(16.0)
                .monospace()
                .strong()
                .color(value_color),
        );
    });
}

fn stat_badge(ui: &mut egui::Ui, colors: Palette, label: &str, value: &str) {
    Frame::default()
        .fill(colors.panel)
        .stroke(Stroke::new(1.0, colors.border))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(label)
                        .size(10.0)
                        .color(colors.text_muted)
                        .strong(),
                );
                ui.label(
                    RichText::new(value)
                        .size(11.0)
                        .color(colors.text)
                        .monospace(),
                );
            });
        });
}

fn warn_box(ui: &mut egui::Ui, color: Color32, text: &str) {
    Frame::default()
        .fill(color.gamma_multiply(0.15))
        .stroke(Stroke::new(1.0, color))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("!").size(14.0).strong().color(color));
                ui.label(RichText::new(text).size(12.0).color(color).monospace());
            });
        });
}

fn input_block(ui: &mut egui::Ui, colors: Palette, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.label(RichText::new(label).size(11.0).color(colors.text_muted));
    add(ui);
    ui.add_space(8.0);
}

fn pair_inputs(
    ui: &mut egui::Ui,
    colors: Palette,
    left_label: &str,
    left_value: &mut String,
    right_label: &str,
    right_value: &mut String,
) {
    ui.columns(2, |cols| {
        cols[0].label(
            RichText::new(left_label)
                .size(11.0)
                .color(colors.text_muted),
        );
        text_field(&mut cols[0], colors, left_value);
        cols[1].label(
            RichText::new(right_label)
                .size(11.0)
                .color(colors.text_muted),
        );
        text_field(&mut cols[1], colors, right_value);
    });
    ui.add_space(8.0);
}

fn text_field(ui: &mut egui::Ui, colors: Palette, value: &mut String) {
    ui.add(
        egui::TextEdit::singleline(value)
            .desired_width(f32::INFINITY)
            .text_color(colors.text)
            .margin(Vec2::new(8.0, 6.0)),
    );
}

fn multiselect_dropdown(
    ui: &mut egui::Ui,
    value: &mut String,
    options: &[FilterOption],
    id_source: &str,
    colors: Palette,
) {
    let mut selected: BTreeSet<String> = value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    let summary = if selected.is_empty() {
        "未选择".to_owned()
    } else {
        options
            .iter()
            .filter(|option| selected.contains(option.key))
            .map(|option| option.zh)
            .take(2)
            .collect::<Vec<_>>()
            .join(", ")
    };

    egui::ComboBox::from_id_salt(id_source)
        .selected_text(RichText::new(summary).color(colors.text).size(12.0))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            ui.set_min_width(260.0);
            ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for option in options {
                    let mut checked = selected.contains(option.key);
                    if ui
                        .checkbox(&mut checked, format!("{} [{}]", option.zh, option.key))
                        .changed()
                    {
                        if checked {
                            selected.insert(option.key.to_owned());
                        } else {
                            selected.remove(option.key);
                        }
                    }
                }
            });
        });

    *value = options
        .iter()
        .filter(|option| selected.contains(option.key))
        .map(|option| option.key)
        .collect::<Vec<_>>()
        .join(",");
}

fn configure_fonts(ctx: &EguiContext) {
    let mut fonts = FontDefinitions::default();
    for path in candidate_font_paths() {
        let Ok(bytes) = fs::read(path) else { continue };
        let font_name = format!("system-cjk-{}", fonts.font_data.len());
        fonts
            .font_data
            .insert(font_name.clone(), FontData::from_owned(bytes).into());
        if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
            family.insert(0, font_name.clone());
        }
        if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
            family.insert(0, font_name);
        }
        ctx.set_fonts(fonts);
        return;
    }
}

fn candidate_font_paths() -> Vec<&'static str> {
    let mut paths = Vec::new();
    if cfg!(target_os = "windows") {
        paths.extend([r"C:\Windows\Fonts\msyh.ttc", r"C:\Windows\Fonts\simhei.ttf"]);
    }
    if cfg!(target_os = "macos") {
        paths.extend([
            "/Library/Fonts/Arial Unicode.ttf",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/System/Library/Fonts/PingFang.ttc",
        ]);
    }
    if cfg!(target_os = "linux") {
        paths.extend([
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJKSC-Regular.otf",
        ]);
    }
    paths
}

fn configure_theme(ctx: &EguiContext, colors: Palette) {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(colors.text);
    visuals.panel_fill = colors.bg;
    visuals.faint_bg_color = colors.panel_alt;
    visuals.extreme_bg_color = colors.preview_bg;

    visuals.window_corner_radius = CornerRadius::same(6);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(4);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(4);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(4);
    visuals.widgets.active.corner_radius = CornerRadius::same(4);
    visuals.widgets.open.corner_radius = CornerRadius::same(4);

    visuals.widgets.noninteractive.bg_fill = colors.panel_alt;
    visuals.widgets.inactive.bg_fill = colors.panel;
    visuals.widgets.hovered.bg_fill = colors.panel_soft;
    visuals.widgets.active.bg_fill = colors.accent_soft;
    visuals.widgets.open.bg_fill = colors.panel_soft;
    visuals.selection.bg_fill = colors.accent_soft;
    visuals.selection.stroke = Stroke::new(1.0, colors.accent);

    let mut style = (*ctx.style()).clone();
    style.visuals = visuals;
    style.spacing.item_spacing = Vec2::new(10.0, 10.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(20.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(13.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::new(12.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::new(12.0, egui::FontFamily::Monospace),
    );
    ctx.set_style(style);
}

fn version_dropdown(ui: &mut egui::Ui, selected: &mut usize, colors: Palette) {
    let current_label = VERSION_ENTRIES
        .get(*selected)
        .map(|e| e.label)
        .unwrap_or("UNKNOWN");

    egui::ComboBox::from_id_salt("version-select")
        .selected_text(RichText::new(current_label).color(colors.text).size(12.0))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            ui.set_min_width(220.0);
            for (i, entry) in VERSION_ENTRIES.iter().enumerate() {
                let label = if entry.warning.is_some() {
                    format!("{} [!]", entry.label)
                } else {
                    entry.label.to_string()
                };
                ui.selectable_value(selected, i, label);
            }
        });
}
