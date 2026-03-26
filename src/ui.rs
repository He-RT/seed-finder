use std::{
    collections::BTreeSet,
    fs,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
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
    config::{search_worker_count, total_seeds, Controls},
    log_diag,
    preview::generate_preview,
    search::run_search,
    templates::{get_templates_by_category, SearchTemplate},
    types::{
        FilterOption, LocatedBiome, LocatedStructure, MatchSummary, PreviewRequest,
        PreviewResponse, WorkerMessage, BIOME_FILTER_OPTIONS, STRUCTURE_FILTER_OPTIONS,
    },
};
use cubiomes::enums::{BiomeID, StructureType};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Theme {
    Dark,
    Light,
    Ocean,
    Forest,
}

impl Theme {
    pub fn colors(&self) -> ThemeColors {
        match self {
            Theme::Dark => ThemeColors {
                bg_primary: Color32::from_rgb(10, 16, 19),
                bg_secondary: Color32::from_rgb(13, 18, 20),
                bg_tertiary: Color32::from_rgb(20, 25, 24),
                text_primary: Color32::from_rgb(240, 239, 232),
                text_secondary: Color32::from_rgb(150, 171, 167),
                accent: Color32::from_rgb(222, 157, 101),
                accent_light: Color32::from_rgb(244, 224, 195),
                border: Color32::from_rgb(57, 70, 65),
                card_bg: Color32::from_rgb(23, 30, 29),
                card_selected: Color32::from_rgb(60, 74, 67),
            },
            Theme::Light => ThemeColors {
                bg_primary: Color32::from_rgb(245, 245, 240),
                bg_secondary: Color32::from_rgb(250, 250, 247),
                bg_tertiary: Color32::from_rgb(240, 238, 230),
                text_primary: Color32::from_rgb(30, 30, 25),
                text_secondary: Color32::from_rgb(100, 100, 95),
                accent: Color32::from_rgb(180, 120, 60),
                accent_light: Color32::from_rgb(220, 180, 130),
                border: Color32::from_rgb(200, 195, 185),
                card_bg: Color32::from_rgb(255, 255, 252),
                card_selected: Color32::from_rgb(230, 220, 200),
            },
            Theme::Ocean => ThemeColors {
                bg_primary: Color32::from_rgb(8, 20, 30),
                bg_secondary: Color32::from_rgb(12, 28, 40),
                bg_tertiary: Color32::from_rgb(18, 38, 55),
                text_primary: Color32::from_rgb(230, 245, 255),
                text_secondary: Color32::from_rgb(130, 180, 210),
                accent: Color32::from_rgb(60, 180, 220),
                accent_light: Color32::from_rgb(150, 210, 240),
                border: Color32::from_rgb(40, 80, 110),
                card_bg: Color32::from_rgb(15, 35, 50),
                card_selected: Color32::from_rgb(30, 60, 85),
            },
            Theme::Forest => ThemeColors {
                bg_primary: Color32::from_rgb(15, 22, 12),
                bg_secondary: Color32::from_rgb(18, 28, 15),
                bg_tertiary: Color32::from_rgb(22, 35, 18),
                text_primary: Color32::from_rgb(235, 245, 230),
                text_secondary: Color32::from_rgb(140, 175, 130),
                accent: Color32::from_rgb(120, 190, 80),
                accent_light: Color32::from_rgb(180, 220, 140),
                border: Color32::from_rgb(50, 75, 45),
                card_bg: Color32::from_rgb(25, 38, 22),
                card_selected: Color32::from_rgb(45, 70, 40),
            },
        }
    }
}

#[derive(Clone)]
pub struct ThemeColors {
    pub bg_primary: Color32,
    pub bg_secondary: Color32,
    pub bg_tertiary: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub accent: Color32,
    pub accent_light: Color32,
    pub border: Color32,
    pub card_bg: Color32,
    pub card_selected: Color32,
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
    pub favorites: Vec<MatchSummary>,
    pub selected_template: Option<String>,
    pub show_template_panel: bool,
    pub clipboard_message: Option<String>,
}

impl SeedFinderApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_fonts(&cc.egui_ctx);

        Self {
            controls: Controls::default(),
            results: Vec::new(),
            selected_result: None,
            search_rx: None,
            preview_rx: None,
            search_cancel: None,
            preview_token: 0,
            preview_texture: None,
            preview_status: "选择右侧结果后显示地图预览".into(),
            status_line: "准备就绪".into(),
            info_line: "左侧输入条件，点击开始筛选".into(),
            progress: None,
            is_searching: false,
            last_error: None,
            theme: Theme::Dark,
            favorites: Vec::new(),
            selected_template: None,
            show_template_panel: false,
            clipboard_message: None,
        }
    }

    pub fn start_search(&mut self, ctx: &EguiContext) {
        log_diag("ui_start_search_clicked");
        let config = match self.controls.to_config() {
            Ok(config) => config,
            Err(err) => {
                log_diag(&format!("ui_start_search_invalid_config: {err}"));
                self.last_error = Some(err.to_string());
                return;
            }
        };

        self.stop_search();
        self.results.clear();
        self.selected_result = None;
        self.preview_rx = None;
        self.preview_texture = None;
        self.preview_status = "搜索完成后，点击下方种子生成地图预览".into();
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
        log_diag(&format!(
            "search_spawn_thread range={}..={} version={}",
            config.seed_start, config.seed_end, config.version_label
        ));

        thread::spawn(move || {
            log_diag("search_thread_enter");
            let result = run_search(config, cancel_clone, tx.clone());
            if let Err(err) = result {
                log_diag(&format!("search_thread_error: {err}"));
                let _ = tx.send(WorkerMessage::Error(err.to_string()));
            }
            log_diag("search_thread_exit");
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
        self.controls.version = template.config.version.clone();
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
    }

    pub fn copy_to_clipboard(&mut self, text: String) {
        self.clipboard_message = Some(format!("请手动复制: {}", text));
    }

    pub fn clear_results(&mut self) {
        self.stop_search();
        self.results.clear();
        self.selected_result = None;
        self.preview_rx = None;
        self.preview_texture = None;
        self.preview_status = "选择右侧结果后显示地图预览".into();
        self.status_line = "结果已清空".into();
        self.info_line.clear();
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

            if let Some(terrain) = summary.terrain.as_ref() {
                output.push_str(&format!(
                    "  Terrain: avg={:.1}, min={:.1}, max={:.1}, relief={:.1}\n",
                    terrain.avg_height, terrain.min_height, terrain.max_height, terrain.relief
                ));
            }

            for (biome, hit) in &summary.biomes {
                output.push_str(&format!(
                    "  Biome: {} at ({}, {}) {:.0}m\n",
                    biome.to_mc_biome_str(summary.version),
                    hit.position.x,
                    hit.position.z,
                    hit.distance
                ));
            }

            for (structure, hit) in &summary.structures {
                output.push_str(&format!(
                    "  Structure: {} at ({}, {}) {:.0}m\n",
                    structure, hit.position.x, hit.position.z, hit.distance
                ));
            }
            output.push('\n');
        }

        output
    }

    pub fn receive_worker_messages(&mut self, _ctx: &EguiContext) {
        let mut should_clear_receiver = false;

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
                        self.info_line = format!("当前种子 {current_seed}，已命中 {found}");
                    }
                    WorkerMessage::Match(summary) => {
                        self.results.push(summary);
                        self.status_line = format!("已找到 {} 个命中种子", self.results.len());
                        self.info_line = "点击下方结果可切换地图预览".into();
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
                            format!("已停止，累计检查 {searched} 个种子")
                        } else {
                            format!("搜索完成，共命中 {found} 个种子")
                        };
                        self.info_line = if found == 0 {
                            "没有找到符合条件的种子".into()
                        } else {
                            "点击下方结果可切换地图预览".into()
                        };
                        should_clear_receiver = true;
                    }
                    WorkerMessage::Error(err) => {
                        self.is_searching = false;
                        self.search_cancel = None;
                        self.progress = None;
                        self.last_error = Some(err);
                        self.status_line = "搜索失败".into();
                        should_clear_receiver = true;
                    }
                }
            }
        }

        if should_clear_receiver {
            self.search_rx = None;
        }
    }

    pub fn receive_preview_messages(&mut self, ctx: &EguiContext) {
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
        self.preview_status = if self.is_searching {
            format!(
                "正在生成种子 {} 的地图预览，搜索会继续进行",
                self.results[index].seed
            )
        } else {
            format!("正在切换到种子 {} 的地图预览", self.results[index].seed)
        };
        self.start_preview(self.results[index].clone(), ctx);
    }

    pub fn start_preview(&mut self, summary: MatchSummary, ctx: &EguiContext) {
        log_diag(&format!("preview_requested seed={}", summary.seed));
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
        self.preview_status = format!("正在生成种子 {} 的地图", summary.seed);

        let request = PreviewRequest {
            token: self.preview_token,
            summary,
            radius,
            image_size,
        };

        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        thread::spawn(move || {
            log_diag(&format!(
                "preview_thread_enter seed={}",
                request.summary.seed
            ));
            let image = generate_preview(&request).map_err(|err| err.to_string());
            let _ = tx.send(PreviewResponse {
                token: request.token,
                image,
            });
            log_diag(&format!(
                "preview_thread_exit seed={}",
                request.summary.seed
            ));
            ctx.request_repaint();
        });

        self.preview_rx = Some(rx);
    }

    pub fn draw_ui(&mut self, ctx: &EguiContext) {
        let colors = self.theme.colors();
        configure_theme_with_colors(ctx, &colors);

        self.draw_left_panel(ctx, &colors);
        self.draw_main_panel(ctx, &colors);
    }

    fn draw_left_panel(&mut self, ctx: &EguiContext, colors: &ThemeColors) {
        egui::SidePanel::left("controls-panel")
            .exact_width(348.0)
            .frame(
                Frame::default()
                    .fill(colors.bg_secondary)
                    .inner_margin(Margin::same(14)),
            )
            .show(ctx, |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        hero_panel(
                            ui,
                            "Seed Finder",
                            "Java 版地形与结构筛种工作台",
                            &self.status_line,
                            colors,
                        );

                        ui.add_space(14.0);
                        panel_section(ui, "运行状态", colors, |ui| {
                            ui.label(
                                RichText::new(&self.status_line)
                                    .size(16.0)
                                    .color(colors.text_primary)
                                    .strong(),
                            );
                            if !self.info_line.is_empty() {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(&self.info_line)
                                        .size(13.0)
                                        .color(colors.text_secondary),
                                );
                            }
                            ui.add_space(10.0);
                            ui.horizontal_wrapped(|ui| {
                                stat_pill(ui, "命中", self.results.len().to_string(), colors);
                                stat_pill(
                                    ui,
                                    "当前版本",
                                    self.controls.version.trim().to_owned(),
                                    colors,
                                );
                                if let Some((searched, total, _, _)) = self.progress {
                                    stat_pill(ui, "进度", format!("{searched}/{total}"), colors);
                                }
                            });
                            if let Some((searched, total, found, current_seed)) = self.progress {
                                ui.add_space(10.0);
                                let progress = if total == 0 {
                                    0.0
                                } else {
                                    searched as f32 / total as f32
                                };
                                let progress_width = ui.available_width().max(120.0);
                                ui.add(
                                    egui::ProgressBar::new(progress)
                                        .desired_width(progress_width)
                                        .fill(colors.accent)
                                        .text(format!("{searched}/{total} seeds")),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!(
                                        "当前 {}，已命中 {}",
                                        current_seed, found
                                    ))
                                    .size(12.5)
                                    .color(colors.text_secondary),
                                );
                            }
                            if let Some(error) = &self.last_error {
                                ui.add_space(8.0);
                                ui.colored_label(Color32::from_rgb(255, 134, 116), error);
                            }
                        });

                        ui.add_space(12.0);
                        panel_section(ui, "主题切换", colors, |ui| {
                            ui.horizontal(|ui| {
                                for theme in
                                    [Theme::Dark, Theme::Light, Theme::Ocean, Theme::Forest]
                                {
                                    let label = match theme {
                                        Theme::Dark => "深色",
                                        Theme::Light => "浅色",
                                        Theme::Ocean => "海洋",
                                        Theme::Forest => "森林",
                                    };
                                    let is_current = self.theme == theme;
                                    let mut btn = Button::new(RichText::new(label).size(12.0));
                                    if is_current {
                                        btn = btn.fill(colors.accent);
                                    }
                                    if ui.add(btn).clicked() {
                                        self.theme = theme;
                                    }
                                }
                            });
                        });

                        ui.add_space(12.0);
                        panel_section(ui, "预设模板", colors, |ui| {
                            let templates_by_category = get_templates_by_category();
                            for (category, templates) in templates_by_category {
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        category.icon(),
                                        category.display()
                                    ))
                                    .size(12.0)
                                    .color(colors.accent)
                                    .strong(),
                                );
                                ui.add_space(4.0);
                                for template in templates {
                                    let is_selected =
                                        self.selected_template.as_deref() == Some(template.id);
                                    let mut btn =
                                        Button::new(RichText::new(template.name).size(11.0));
                                    if is_selected {
                                        btn = btn.fill(colors.accent);
                                    }
                                    if ui.add(btn).on_hover_text(template.description).clicked() {
                                        self.apply_template(&template);
                                    }
                                }
                                ui.add_space(6.0);
                            }
                        });

                        ui.add_space(12.0);
                        panel_section(ui, "搜索范围", colors, |ui| {
                            field_row(ui, "起始种子", &mut self.controls.seed_start, colors);
                            field_row(ui, "结束种子", &mut self.controls.seed_end, colors);
                            numeric_pair(
                                ui,
                                "版本",
                                &mut self.controls.version,
                                "命中上限",
                                &mut self.controls.limit,
                                colors,
                            );
                        });

                        ui.add_space(12.0);
                        panel_section(ui, "生物群系约束", colors, |ui| {
                            multiselect_dropdown(
                                ui,
                                "需要生物群系",
                                &mut self.controls.require_biome,
                                BIOME_FILTER_OPTIONS,
                                "require_biome_dropdown",
                                colors,
                            );
                            multiselect_dropdown(
                                ui,
                                "排除生物群系",
                                &mut self.controls.forbid_biome,
                                BIOME_FILTER_OPTIONS,
                                "forbid_biome_dropdown",
                                colors,
                            );
                            numeric_pair(
                                ui,
                                "搜索半径",
                                &mut self.controls.biome_radius,
                                "采样步长",
                                &mut self.controls.biome_step,
                                colors,
                            );
                        });

                        ui.add_space(12.0);
                        panel_section(ui, "结构与地形", colors, |ui| {
                            multiselect_dropdown(
                                ui,
                                "需要结构",
                                &mut self.controls.require_structure,
                                STRUCTURE_FILTER_OPTIONS,
                                "require_structure_dropdown",
                                colors,
                            );
                            numeric_pair(
                                ui,
                                "结构半径",
                                &mut self.controls.structure_radius,
                                "地形半径",
                                &mut self.controls.terrain_radius,
                                colors,
                            );
                            numeric_pair(
                                ui,
                                "最小平均高",
                                &mut self.controls.min_avg_height,
                                "最大平均高",
                                &mut self.controls.max_avg_height,
                                colors,
                            );
                            field_row(ui, "最大高差", &mut self.controls.max_relief, colors);
                        });

                        ui.add_space(12.0);
                        panel_section(ui, "预览输出", colors, |ui| {
                            numeric_pair(
                                ui,
                                "预览半径",
                                &mut self.controls.preview_radius,
                                "图像尺寸",
                                &mut self.controls.preview_image_size,
                                colors,
                            );
                        });

                        ui.add_space(14.0);
                        Frame::default()
                            .fill(colors.bg_tertiary)
                            .stroke(Stroke::new(1.0, colors.border))
                            .corner_radius(CornerRadius::same(18))
                            .inner_margin(Margin::same(12))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let search_label = if self.is_searching {
                                        "搜索进行中"
                                    } else {
                                        "开始筛选"
                                    };
                                    if ui
                                        .add_sized(
                                            [136.0, 46.0],
                                            Button::new(
                                                RichText::new(search_label)
                                                    .size(15.0)
                                                    .strong()
                                                    .color(Color32::from_rgb(20, 24, 24)),
                                            )
                                            .fill(colors.accent),
                                        )
                                        .clicked()
                                        && !self.is_searching
                                    {
                                        self.start_search(ctx);
                                    }

                                    if ui
                                        .add_enabled(
                                            self.is_searching,
                                            Button::new(RichText::new("停止").size(15.0))
                                                .min_size(Vec2::new(84.0, 46.0)),
                                        )
                                        .clicked()
                                    {
                                        self.stop_search();
                                        self.status_line = "已请求停止搜索".into();
                                    }

                                    if ui
                                        .add_sized(
                                            [84.0, 46.0],
                                            Button::new(RichText::new("清空").size(15.0)),
                                        )
                                        .clicked()
                                    {
                                        self.clear_results();
                                    }
                                });

                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    if ui
                                        .add_sized(
                                            [100.0, 36.0],
                                            Button::new(RichText::new("导出结果").size(13.0)),
                                        )
                                        .clicked()
                                        && !self.results.is_empty()
                                    {
                                        let export = self.export_results();
                                        if let Some(path) = rfd::FileDialog::new()
                                            .add_filter("Text", &["txt"])
                                            .save_file()
                                        {
                                            let _ = fs::write(path, export);
                                        }
                                    }

                                    if ui
                                        .add_sized(
                                            [100.0, 36.0],
                                            Button::new(RichText::new("收藏列表").size(13.0)),
                                        )
                                        .clicked()
                                    {
                                        if !self.favorites.is_empty() {
                                            self.results = self.favorites.clone();
                                            self.selected_result = None;
                                            self.preview_texture = None;
                                            self.preview_status =
                                                format!("已加载 {} 个收藏种子", self.results.len());
                                        }
                                    }
                                });
                            });
                        ui.add_space(10.0);
                    });
            });
    }

    fn draw_main_panel(&mut self, ctx: &EguiContext, colors: &ThemeColors) {
        egui::CentralPanel::default()
            .frame(
                Frame::default()
                    .fill(colors.bg_primary)
                    .inner_margin(Margin::same(18)),
            )
            .show(ctx, |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            self.draw_preview_area(ui, colors);
                            ui.add_space(18.0);
                            self.draw_results_area(ui, ctx, colors);
                        });
                    });
            });
    }

    fn draw_preview_area(&mut self, ui: &mut egui::Ui, colors: &ThemeColors) {
        Frame::default()
            .fill(colors.bg_secondary)
            .stroke(Stroke::new(1.0, colors.border))
            .corner_radius(CornerRadius::same(22))
            .inner_margin(Margin::same(18))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("Terrain Canvas")
                                .size(26.0)
                                .color(colors.text_primary)
                                .strong(),
                        );
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new("出生点周边地形、生物群系与结构叠加视图")
                                .size(13.0)
                                .color(colors.text_secondary),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        info_badge(ui, &self.preview_status, colors);
                    });
                });
                ui.add_space(14.0);
                let available = ui.available_width();
                let preview_height = ui.available_height().clamp(320.0, 460.0);
                ui.columns(2, |columns| {
                    let dossier_width = 320.0;
                    let map_width = (available - dossier_width - 20.0).max(320.0);
                    Frame::default()
                        .fill(colors.bg_primary)
                        .corner_radius(CornerRadius::same(18))
                        .stroke(Stroke::new(1.0, colors.border))
                        .inner_margin(Margin::same(14))
                        .show(&mut columns[0], |ui| {
                            ui.set_min_size(Vec2::new(map_width, preview_height));
                            ui.centered_and_justified(|ui| {
                                if let Some(texture) = &self.preview_texture {
                                    let size = texture.size_vec2();
                                    let scale = ((map_width - 32.0) / size.x)
                                        .min((preview_height - 32.0) / size.y)
                                        .max(0.1);
                                    ui.image((texture.id(), size * scale));
                                } else {
                                    ui.vertical_centered(|ui| {
                                        if self.selected_result.is_some() {
                                            ui.add(egui::Spinner::new().size(24.0));
                                            ui.add_space(10.0);
                                        }
                                        ui.label(
                                            RichText::new(&self.preview_status)
                                                .size(15.0)
                                                .color(colors.text_secondary),
                                        );
                                    });
                                }
                            });
                        });
                    Frame::default()
                        .fill(colors.bg_tertiary)
                        .corner_radius(CornerRadius::same(18))
                        .stroke(Stroke::new(1.0, colors.border))
                        .inner_margin(Margin::same(14))
                        .show(&mut columns[1], |ui| {
                            ui.set_min_size(Vec2::new(dossier_width, preview_height));
                            ui.label(
                                RichText::new("Seed Dossier")
                                    .size(18.0)
                                    .color(colors.text_primary)
                                    .strong(),
                            );
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new("当前选中种子的关键地形与结构情报")
                                    .size(13.0)
                                    .color(colors.text_secondary),
                            );
                            ui.add_space(10.0);
                            ui.horizontal_wrapped(|ui| {
                                legend_chip(ui, Color32::WHITE, "出生点", colors);
                                legend_chip(ui, colors.accent, "结构", colors);
                                legend_chip(ui, Color32::from_rgb(93, 215, 198), "命中生物群系", colors);
                            });
                            ui.add_space(12.0);
                            if let Some(index) = self.selected_result {
                                self.draw_seed_dossier(ui, &self.results[index], colors);
                            } else {
                                info_warning(ui, "从下方 Matches 里点一条种子后，这里会显示其出生点、版本、地形指标、生物群系和结构摘要。", colors);
                            }
                        });
                });
            });
    }

    fn draw_seed_dossier(&self, ui: &mut egui::Ui, summary: &MatchSummary, colors: &ThemeColors) {
        ui.label(
            RichText::new(format!("Seed {}", summary.seed))
                .size(24.0)
                .color(colors.text_primary)
                .strong(),
        );
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            detail_chip(
                ui,
                format!("Spawn ({}, {})", summary.spawn.x, summary.spawn.z),
                colors,
            );
            detail_chip(ui, format!("Version {}", summary.version_label), colors);
            if summary.is_favorite {
                detail_chip(ui, "已收藏".into(), colors);
            }
        });

        if let Some(score) = &summary.score {
            ui.add_space(12.0);
            let (r, g, b) = score.grade.color();
            Frame::default()
                .fill(Color32::from_rgb(r, g, b))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::symmetric(12, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "评分: {} ({:.1}分)",
                                score.grade.display(),
                                score.overall
                            ))
                            .size(14.0)
                            .color(Color32::BLACK)
                            .strong(),
                        );
                    });
                });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                stat_pill(ui, "资源", format!("{:.0}", score.resource_score), colors);
                stat_pill(ui, "安全", format!("{:.0}", score.safety_score), colors);
                stat_pill(ui, "群系", format!("{:.0}", score.biome_diversity), colors);
                stat_pill(ui, "结构", format!("{:.0}", score.structure_score), colors);
                stat_pill(ui, "农场", format!("{:.0}", score.farming_score), colors);
            });
        }

        if let Some(spawn_analysis) = &summary.spawn_analysis {
            ui.add_space(12.0);
            ui.label(
                RichText::new("出生点分析")
                    .size(13.0)
                    .color(colors.text_secondary)
                    .strong(),
            );
            ui.add_space(4.0);
            if spawn_analysis.is_safe {
                ui.label(
                    RichText::new("✓ 安全出生点")
                        .size(12.0)
                        .color(Color32::from_rgb(100, 200, 100)),
                );
            } else {
                ui.label(
                    RichText::new("⚠ 出生点可能有风险")
                        .size(12.0)
                        .color(Color32::from_rgb(255, 150, 100)),
                );
            }
            if !spawn_analysis.issues.is_empty() {
                for issue in &spawn_analysis.issues {
                    ui.label(
                        RichText::new(format!("  • {}", issue))
                            .size(11.0)
                            .color(Color32::from_rgb(255, 180, 150)),
                    );
                }
            }
            if !spawn_analysis.advantages.is_empty() {
                for adv in &spawn_analysis.advantages {
                    ui.label(
                        RichText::new(format!("  ✓ {}", adv))
                            .size(11.0)
                            .color(Color32::from_rgb(150, 220, 150)),
                    );
                }
            }
            if !spawn_analysis.nearby_resources.animals.is_empty() {
                ui.add_space(4.0);
                let animals: Vec<_> = spawn_analysis
                    .nearby_resources
                    .animals
                    .iter()
                    .map(|a| a.display())
                    .collect();
                ui.label(
                    RichText::new(format!("附近动物: {}", animals.join(", ")))
                        .size(11.0)
                        .color(colors.text_secondary),
                );
            }
        }

        if let Some(terrain) = summary.terrain.as_ref() {
            ui.add_space(12.0);
            ui.label(
                RichText::new("Terrain Metrics")
                    .size(13.0)
                    .color(colors.text_secondary)
                    .strong(),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                stat_pill(ui, "AVG", format!("{:.1}", terrain.avg_height), colors);
                stat_pill(ui, "MIN", format!("{:.1}", terrain.min_height), colors);
                stat_pill(ui, "MAX", format!("{:.1}", terrain.max_height), colors);
                stat_pill(ui, "RELIEF", format!("{:.1}", terrain.relief), colors);
            });
        }
        if !summary.biomes.is_empty() {
            ui.add_space(12.0);
            ui.label(
                RichText::new("Biomes")
                    .size(13.0)
                    .color(colors.text_secondary)
                    .strong(),
            );
            ui.add_space(5.0);
            ui.label(format_hits(
                summary.version,
                &summary.biomes,
                &summary.structures,
            ));
        }
        if !summary.structures.is_empty() {
            ui.add_space(12.0);
            ui.label(
                RichText::new("Structures")
                    .size(13.0)
                    .color(colors.text_secondary)
                    .strong(),
            );
            ui.add_space(5.0);
            ui.label(format_structure_hits(&summary.structures));
        }

        ui.add_space(12.0);
        ui.label(
            RichText::new("快速操作")
                .size(13.0)
                .color(colors.text_secondary)
                .strong(),
        );
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            let tp_cmd = format_teleport_command(summary.spawn, "overworld");
            let coord = format_coordinate_copy(summary.spawn);
            ui.label(
                RichText::new(format!("坐标: {}", coord))
                    .size(11.0)
                    .color(colors.text_secondary),
            );
            ui.label(
                RichText::new(format!("TP: {}", tp_cmd))
                    .size(10.0)
                    .color(Color32::from_rgb(100, 100, 100)),
            );
        });
        ui.add_space(4.0);
        ui.label(
            RichText::new(format_nether_info(summary.spawn))
                .size(11.0)
                .color(colors.text_secondary),
        );

        if let Some(farming) = &summary.farming_analysis {
            if !farming.iron_golem_villages.is_empty()
                || !farming.special_biomes_for_farms.is_empty()
            {
                ui.add_space(12.0);
                ui.label(
                    RichText::new("农场潜力")
                        .size(13.0)
                        .color(colors.text_secondary)
                        .strong(),
                );
                ui.add_space(4.0);
                if farming.iron_golem_villages.len() >= 2 {
                    ui.label(
                        RichText::new(format!(
                            "✓ {} 个村庄可用于刷铁机",
                            farming.iron_golem_villages.len()
                        ))
                        .size(11.0)
                        .color(Color32::from_rgb(100, 200, 100)),
                    );
                }
                for (_biome, desc) in &farming.special_biomes_for_farms {
                    ui.label(
                        RichText::new(format!("• {}", desc))
                            .size(11.0)
                            .color(colors.text_secondary),
                    );
                }
            }
        }

        if let Some(version_warning) = &summary.version_warning {
            ui.add_space(12.0);
            info_warning(ui, version_warning, colors);
        }
    }

    fn draw_results_area(&mut self, ui: &mut egui::Ui, ctx: &EguiContext, colors: &ThemeColors) {
        Frame::default()
            .fill(colors.bg_secondary)
            .stroke(Stroke::new(1.0, colors.border))
            .corner_radius(CornerRadius::same(20))
            .inner_margin(Margin::same(16))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Matches")
                            .size(22.0)
                            .color(colors.text_primary)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{} results", self.results.len()))
                                .size(13.0)
                                .color(colors.text_secondary),
                        );
                    });
                });
                ui.add_space(6.0);
                ui.label(
                    RichText::new("点击任意种子，右侧地图预览会立即切换")
                        .size(12.5)
                        .color(colors.text_secondary),
                );
                ui.add_space(10.0);

                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(420.0)
                    .show(ui, |ui| {
                        for index in 0..self.results.len() {
                            let selected = self.selected_result == Some(index);
                            let card = result_row(&self.results[index]);
                            let fill = if selected {
                                colors.card_selected
                            } else {
                                colors.card_bg
                            };
                            let mut preview_clicked = false;
                            let mut favorite_clicked = false;

                            let response = Frame::default()
                                .fill(fill)
                                .stroke(Stroke::new(
                                    1.0,
                                    if selected {
                                        colors.accent
                                    } else {
                                        colors.border
                                    },
                                ))
                                .corner_radius(CornerRadius::same(14))
                                .inner_margin(Margin::same(12))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.set_min_height(68.0);
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(format!("Seed {}", card.seed))
                                                        .size(17.0)
                                                        .color(colors.text_primary)
                                                        .strong(),
                                                );
                                                if card.is_favorite {
                                                    ui.label(
                                                        RichText::new("★")
                                                            .size(14.0)
                                                            .color(colors.accent),
                                                    );
                                                }
                                            });
                                            ui.add_space(2.0);
                                            ui.label(
                                                RichText::new(card.subtitle)
                                                    .size(13.0)
                                                    .color(colors.text_secondary),
                                            );
                                        });
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .add(
                                                        Button::new(
                                                            RichText::new("预览")
                                                                .size(13.0)
                                                                .strong(),
                                                        )
                                                        .fill(colors.accent)
                                                        .stroke(Stroke::new(
                                                            1.0,
                                                            colors.accent_light,
                                                        ))
                                                        .min_size(Vec2::new(72.0, 34.0)),
                                                    )
                                                    .clicked()
                                                {
                                                    preview_clicked = true;
                                                }
                                                if ui
                                                    .add(
                                                        Button::new(
                                                            RichText::new(if card.is_favorite {
                                                                "取消收藏"
                                                            } else {
                                                                "收藏"
                                                            })
                                                            .size(12.0),
                                                        )
                                                        .min_size(Vec2::new(60.0, 28.0)),
                                                    )
                                                    .clicked()
                                                {
                                                    favorite_clicked = true;
                                                }
                                                if let Some(relief) = card.relief {
                                                    detail_chip(
                                                        ui,
                                                        format!("relief {:.1}", relief),
                                                        colors,
                                                    );
                                                }
                                                if let Some(structure) = card.first_structure {
                                                    detail_chip(ui, structure, colors);
                                                }
                                            },
                                        );
                                    });
                                })
                                .response
                                .interact(egui::Sense::click());

                            if response.clicked() || preview_clicked {
                                self.select_result(index, ctx);
                            }

                            if favorite_clicked {
                                self.toggle_favorite(index);
                            }

                            ui.add_space(8.0);
                        }
                    });
            });
    }
}

impl eframe::App for SeedFinderApp {
    fn update(&mut self, ctx: &EguiContext, _frame: &mut eframe::Frame) {
        self.receive_worker_messages(ctx);
        self.receive_preview_messages(ctx);

        if self.is_searching {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        self.draw_ui(ctx);
    }
}

fn configure_fonts(ctx: &EguiContext) {
    let mut fonts = FontDefinitions::default();

    for path in candidate_font_paths() {
        let Ok(bytes) = fs::read(path) else {
            continue;
        };

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
        paths.extend([
            r"C:\Windows\Fonts\msyh.ttc",
            r"C:\Windows\Fonts\msyhbd.ttc",
            r"C:\Windows\Fonts\simhei.ttf",
            r"C:\Windows\Fonts\simsun.ttc",
            r"C:\Windows\Fonts\Deng.ttf",
            r"C:\Windows\Fonts\YuGothM.ttc",
            r"C:\Windows\Fonts\Arialuni.ttf",
        ]);
    }

    if cfg!(target_os = "macos") {
        paths.extend([
            "/Library/Fonts/Arial Unicode.ttf",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
            "/System/Library/Fonts/STHeiti Medium.ttc",
            "/System/Library/Fonts/PingFang.ttc",
        ]);
    }

    if cfg!(target_os = "linux") {
        paths.extend([
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJKSC-Regular.otf",
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/truetype/arphic/uming.ttc",
        ]);
    }

    paths
}

fn configure_theme_with_colors(ctx: &EguiContext, colors: &ThemeColors) {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(colors.text_primary);
    visuals.panel_fill = colors.bg_primary;
    visuals.faint_bg_color = colors.bg_tertiary;
    visuals.extreme_bg_color = colors.bg_primary;
    visuals.widgets.noninteractive.bg_fill = colors.bg_tertiary;
    visuals.widgets.inactive.bg_fill = colors.card_bg;
    visuals.widgets.hovered.bg_fill = colors.card_selected;
    visuals.widgets.active.bg_fill = colors.accent;
    visuals.widgets.open.bg_fill = colors.bg_tertiary;
    visuals.selection.bg_fill = colors.accent;
    visuals.selection.stroke = Stroke::new(1.0, colors.accent_light);
    visuals.window_corner_radius = CornerRadius::same(20);

    let mut style = (*ctx.style()).clone();
    style.visuals = visuals;
    style.spacing.item_spacing = Vec2::new(10.0, 12.0);
    style.spacing.button_padding = Vec2::new(14.0, 12.0);
    style.spacing.indent = 14.0;
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(26.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(14.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::new(14.5, egui::FontFamily::Proportional),
    );
    ctx.set_style(style);
}

fn hero_panel(ui: &mut egui::Ui, title: &str, subtitle: &str, status: &str, colors: &ThemeColors) {
    Frame::default()
        .fill(colors.bg_tertiary)
        .stroke(Stroke::new(1.0, colors.border))
        .corner_radius(CornerRadius::same(22))
        .inner_margin(Margin::same(16))
        .show(ui, |ui| {
            ui.label(
                RichText::new(title)
                    .size(31.0)
                    .color(colors.text_primary)
                    .strong(),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(subtitle)
                    .size(13.5)
                    .color(colors.text_secondary),
            );
            ui.add_space(12.0);
            info_badge(ui, status, colors);
        });
}

fn panel_section(
    ui: &mut egui::Ui,
    title: &str,
    colors: &ThemeColors,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    Frame::default()
        .fill(colors.bg_tertiary)
        .stroke(Stroke::new(1.0, colors.border))
        .corner_radius(CornerRadius::same(18))
        .inner_margin(Margin::same(14))
        .show(ui, |ui| {
            ui.label(
                RichText::new(title)
                    .size(12.0)
                    .color(colors.accent)
                    .strong(),
            );
            ui.add_space(8.0);
            add_contents(ui);
        });
}

fn stat_pill(ui: &mut egui::Ui, label: &str, value: String, colors: &ThemeColors) {
    Frame::default()
        .fill(colors.card_bg)
        .stroke(Stroke::new(1.0, colors.border))
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(label)
                        .size(11.0)
                        .color(colors.text_secondary)
                        .strong(),
                );
                ui.label(
                    RichText::new(value)
                        .size(12.5)
                        .color(colors.text_primary)
                        .strong(),
                );
            });
        });
}

fn info_badge(ui: &mut egui::Ui, text: &str, colors: &ThemeColors) {
    Frame::default()
        .fill(colors.bg_tertiary)
        .stroke(Stroke::new(1.0, colors.accent))
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(12, 7))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(12.5).color(colors.accent_light));
        });
}

fn info_warning(ui: &mut egui::Ui, text: &str, _colors: &ThemeColors) {
    Frame::default()
        .fill(Color32::from_rgb(64, 46, 31))
        .stroke(Stroke::new(1.0, Color32::from_rgb(146, 114, 80)))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .size(12.5)
                    .color(Color32::from_rgb(247, 218, 189)),
            );
        });
}

fn multiselect_dropdown(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    options: &[FilterOption],
    id_source: &str,
    colors: &ThemeColors,
) {
    ui.label(RichText::new(label).size(12.5).color(colors.text_secondary));

    let mut selected = parse_selection_set(value);
    let summary = selection_summary(&selected, options);

    egui::ComboBox::from_id_salt(id_source)
        .selected_text(summary)
        .width(ui.available_width().max(180.0))
        .show_ui(ui, |ui| {
            ui.set_min_width(260.0);
            ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for option in options {
                    let mut checked = selected.contains(option.key);
                    let text = format!("{} · {}", option.zh, option.key);
                    if ui.checkbox(&mut checked, text).changed() {
                        if checked {
                            selected.insert(option.key.to_owned());
                        } else {
                            selected.remove(option.key);
                        }
                    }
                }
            });
        });

    *value = format_selection_set(&selected, options);
}

fn parse_selection_set(raw: &str) -> BTreeSet<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn format_selection_set(selected: &BTreeSet<String>, options: &[FilterOption]) -> String {
    options
        .iter()
        .filter(|option| selected.contains(option.key))
        .map(|option| option.key)
        .collect::<Vec<_>>()
        .join(",")
}

fn selection_summary(selected: &BTreeSet<String>, options: &[FilterOption]) -> String {
    if selected.is_empty() {
        return "未选择".into();
    }

    let labels = options
        .iter()
        .filter(|option| selected.contains(option.key))
        .map(|option| option.zh)
        .take(2)
        .collect::<Vec<_>>();

    if selected.len() <= 2 {
        labels.join("、")
    } else {
        format!("{} 等 {} 项", labels.join("、"), selected.len())
    }
}

fn field_row(ui: &mut egui::Ui, label: &str, value: &mut String, colors: &ThemeColors) {
    ui.label(RichText::new(label).size(12.5).color(colors.text_secondary));
    ui.add(
        egui::TextEdit::singleline(value)
            .desired_width(f32::INFINITY)
            .margin(Vec2::new(12.0, 10.0)),
    );
}

fn numeric_pair(
    ui: &mut egui::Ui,
    left_label: &str,
    left_value: &mut String,
    right_label: &str,
    right_value: &mut String,
    colors: &ThemeColors,
) {
    ui.columns(2, |columns| {
        field_row(&mut columns[0], left_label, left_value, colors);
        field_row(&mut columns[1], right_label, right_value, colors);
    });
}

fn legend_chip(ui: &mut egui::Ui, color: Color32, label: &str, colors: &ThemeColors) {
    Frame::default()
        .fill(colors.card_bg)
        .stroke(Stroke::new(1.0, colors.border))
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(9.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 4.0, color);
                ui.label(RichText::new(label).size(12.5).color(colors.text_primary));
            });
        });
}

fn detail_chip(ui: &mut egui::Ui, label: String, colors: &ThemeColors) {
    Frame::default()
        .fill(colors.card_bg)
        .stroke(Stroke::new(1.0, colors.border))
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(12.5).color(colors.text_primary));
        });
}

struct ResultRow<'a> {
    seed: i64,
    subtitle: String,
    first_structure: Option<String>,
    relief: Option<f32>,
    is_favorite: bool,
    _marker: std::marker::PhantomData<&'a ()>,
}

fn result_row(summary: &MatchSummary) -> ResultRow<'_> {
    let subtitle = format!("spawn ({}, {})", summary.spawn.x, summary.spawn.z);
    let first_structure = summary
        .structures
        .first()
        .map(|(structure, hit)| format!("{structure} {:.0}m", hit.distance));
    let relief = summary.terrain.as_ref().map(|terrain| terrain.relief);

    ResultRow {
        seed: summary.seed,
        subtitle,
        first_structure,
        relief,
        is_favorite: summary.is_favorite,
        _marker: std::marker::PhantomData,
    }
}

fn format_hits(
    version: cubiomes::enums::MCVersion,
    biomes: &[(BiomeID, LocatedBiome)],
    _structures: &[(StructureType, LocatedStructure)],
) -> String {
    biomes
        .iter()
        .map(|(biome, hit): &(_, _)| {
            format!(
                "{} @ ({}, {}) {:.0}m",
                biome.to_mc_biome_str(version),
                hit.position.x,
                hit.position.z,
                hit.distance
            )
        })
        .collect::<Vec<_>>()
        .join("  |  ")
}

fn format_structure_hits(structures: &[(StructureType, LocatedStructure)]) -> String {
    structures
        .iter()
        .map(|(structure, hit)| {
            format!(
                "{} @ ({}, {}) {:.0}m",
                structure, hit.position.x, hit.position.z, hit.distance
            )
        })
        .collect::<Vec<_>>()
        .join("  |  ")
}
