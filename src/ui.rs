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
    analysis::{format_nether_info, format_teleport_command},
    config::{total_seeds, Controls},
    preview::generate_preview,
    search::run_search,
    templates::{get_templates_by_category, SearchTemplate},
    types::{
        FilterOption, MatchSummary, PreviewRequest, PreviewResponse, WorkerMessage,
        BIOME_FILTER_OPTIONS, STRUCTURE_FILTER_OPTIONS,
    },
};

const COLOR_BG_DARK: Color32 = Color32::from_rgb(15, 15, 20);
const COLOR_BG_CARD: Color32 = Color32::from_rgb(25, 28, 35);
const COLOR_BG_HOVER: Color32 = Color32::from_rgb(35, 40, 50);
const COLOR_ACCENT: Color32 = Color32::from_rgb(255, 170, 80);
const COLOR_ACCENT_DIM: Color32 = Color32::from_rgb(180, 120, 50);
const COLOR_TEXT: Color32 = Color32::from_rgb(240, 240, 245);
const COLOR_TEXT_DIM: Color32 = Color32::from_rgb(150, 155, 165);
const COLOR_BORDER: Color32 = Color32::from_rgb(50, 55, 65);
const COLOR_SUCCESS: Color32 = Color32::from_rgb(80, 200, 120);
const COLOR_WARNING: Color32 = Color32::from_rgb(255, 180, 80);
const COLOR_DANGER: Color32 = Color32::from_rgb(255, 100, 100);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppTab {
    Search,
    Templates,
    Favorites,
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
    pub progress: Option<(usize, usize, usize, i64)>,
    pub is_searching: bool,
    pub last_error: Option<String>,
    pub favorites: Vec<MatchSummary>,
    pub selected_template: Option<String>,
    pub current_tab: AppTab,
    pub sidebar_width: f32,
}

impl SeedFinderApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_fonts(&cc.egui_ctx);
        configure_style(&cc.egui_ctx);

        Self {
            controls: Controls::default(),
            results: Vec::new(),
            selected_result: None,
            search_rx: None,
            preview_rx: None,
            search_cancel: None,
            preview_token: 0,
            preview_texture: None,
            progress: None,
            is_searching: false,
            last_error: None,
            favorites: Vec::new(),
            selected_template: None,
            current_tab: AppTab::Templates,
            sidebar_width: 380.0,
        }
    }

    pub fn start_search(&mut self, ctx: &EguiContext) {
        let config = match self.controls.to_config() {
            Ok(config) => config,
            Err(err) => {
                self.last_error = Some(err.to_string());
                return;
            }
        };

        self.stop_search();
        self.results.clear();
        self.selected_result = None;
        self.preview_rx = None;
        self.preview_texture = None;
        self.progress = Some((0, total_seeds(&config), 0, config.seed_start));

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
        self.current_tab = AppTab::Search;
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
        output.push_str("# Minecraft Seed Finder Export\n\n");

        for summary in &self.results {
            output.push_str(&format!("Seed: {}\n", summary.seed));
            output.push_str(&format!(
                "  Spawn: ({}, {})\n",
                summary.spawn.x, summary.spawn.z
            ));

            if let Some(score) = &summary.score {
                output.push_str(&format!(
                    "  Rating: {} ({:.1}/100)\n",
                    score.grade.display(),
                    score.overall
                ));
            }

            for (biome, hit) in &summary.biomes {
                output.push_str(&format!(
                    "  Biome: {} @ ({}, {}) {:.0}m\n",
                    biome.to_mc_biome_str(summary.version),
                    hit.position.x,
                    hit.position.z,
                    hit.distance
                ));
            }

            for (structure, hit) in &summary.structures {
                output.push_str(&format!(
                    "  Structure: {} @ ({}, {}) {:.0}m\n",
                    structure, hit.position.x, hit.position.z, hit.distance
                ));
            }

            let (nx, nz) = (summary.spawn.x / 8, summary.spawn.z / 8);
            output.push_str(&format!("  Nether: ({}, {})\n\n", nx, nz));
        }

        output
    }

    fn receive_messages(&mut self, ctx: &EguiContext) {
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
                    }
                    WorkerMessage::Match(summary) => {
                        self.results.push(summary);
                    }
                    WorkerMessage::Finished { stopped, .. } => {
                        self.is_searching = false;
                        self.search_cancel = None;
                        self.progress = None;
                        if stopped {
                            self.results.sort_by(|a, b| {
                                b.score
                                    .as_ref()
                                    .map(|s| s.overall)
                                    .unwrap_or(0.0)
                                    .partial_cmp(
                                        &a.score.as_ref().map(|s| s.overall).unwrap_or(0.0),
                                    )
                                    .unwrap()
                            });
                        }
                    }
                    WorkerMessage::Error(err) => {
                        self.is_searching = false;
                        self.search_cancel = None;
                        self.progress = None;
                        self.last_error = Some(err);
                    }
                }
            }
        }

        if let Some(rx) = &self.preview_rx {
            while let Ok(message) = rx.try_recv() {
                if message.token == self.preview_token {
                    if let Ok(image) = message.image {
                        self.preview_texture =
                            Some(ctx.load_texture("preview", image, TextureOptions::LINEAR));
                    }
                }
            }
        }
    }

    fn select_result(&mut self, index: usize, ctx: &EguiContext) {
        if index >= self.results.len() {
            return;
        }
        self.selected_result = Some(index);
        self.start_preview(self.results[index].clone(), ctx);
    }

    fn start_preview(&mut self, summary: MatchSummary, ctx: &EguiContext) {
        let Ok(radius) = self.controls.preview_radius() else {
            return;
        };
        let Ok(image_size) = self.controls.preview_image_size() else {
            return;
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
            let image = generate_preview(&request).map_err(|e| e.to_string());
            let _ = tx.send(PreviewResponse {
                token: request.token,
                image,
            });
            ctx.request_repaint();
        });

        self.preview_rx = Some(rx);
    }
}

impl eframe::App for SeedFinderApp {
    fn update(&mut self, ctx: &EguiContext, _frame: &mut eframe::Frame) {
        self.receive_messages(ctx);

        if self.is_searching {
            ctx.request_repaint_after(Duration::from_millis(50));
        }

        self.draw_main_ui(ctx);
    }
}

impl SeedFinderApp {
    fn draw_main_ui(&mut self, ctx: &EguiContext) {
        egui::CentralPanel::default()
            .frame(Frame::default().fill(COLOR_BG_DARK).inner_margin(0.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    self.draw_sidebar(ui, ctx);
                    self.draw_content(ui, ctx);
                });
            });
    }

    fn draw_sidebar(&mut self, ui: &mut egui::Ui, ctx: &EguiContext) {
        let sidebar_width = self.sidebar_width;

        egui::SidePanel::left("sidebar")
            .exact_width(sidebar_width)
            .frame(Frame::default().fill(COLOR_BG_CARD).inner_margin(0.0))
            .show_inside(ui, |ui| {
                self.draw_header(ui);
                self.draw_tabs(ui);
                ui.add_space(8.0);

                match self.current_tab {
                    AppTab::Search => self.draw_search_panel(ui, ctx),
                    AppTab::Templates => self.draw_templates_panel(ui),
                    AppTab::Favorites => self.draw_favorites_panel(ui, ctx),
                }
            });
    }

    fn draw_header(&self, ui: &mut egui::Ui) {
        Frame::default()
            .fill(COLOR_BG_DARK)
            .inner_margin(Margin::symmetric(20, 16))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Seed Finder")
                            .size(22.0)
                            .color(COLOR_ACCENT)
                            .strong(),
                    );
                    ui.label(RichText::new("v2.0").size(12.0).color(COLOR_TEXT_DIM));
                });
                ui.label(
                    RichText::new("Minecraft 种子筛选工具")
                        .size(12.0)
                        .color(COLOR_TEXT_DIM),
                );
            });
    }

    fn draw_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for (tab, label) in [
                (AppTab::Templates, "模板"),
                (AppTab::Search, "搜索"),
                (AppTab::Favorites, "收藏"),
            ] {
                let is_active = self.current_tab == tab;
                let mut btn = Button::new(RichText::new(label).size(13.0));
                if is_active {
                    btn = btn.fill(COLOR_ACCENT).stroke(Stroke::NONE);
                } else {
                    btn = btn.fill(Color32::TRANSPARENT).stroke(Stroke::NONE);
                }
                if ui.add(btn).clicked() {
                    self.current_tab = tab;
                }
            }
        });
        ui.add_space(4.0);
    }

    fn draw_search_panel(&mut self, ui: &mut egui::Ui, ctx: &EguiContext) {
        ScrollArea::vertical().show(ui, |ui| {
            Frame::default()
                .inner_margin(Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    if let Some((searched, total, found, current)) = self.progress {
                        self.draw_progress_section(ui, searched, total, found, current);
                    }

                    if let Some(error) = &self.last_error {
                        ui.colored_label(COLOR_DANGER, error);
                        ui.add_space(8.0);
                    }

                    self.draw_input_section(ui);
                    self.draw_filter_section(ui);
                    self.draw_action_buttons(ui, ctx);
                });
        });
    }

    fn draw_progress_section(
        &self,
        ui: &mut egui::Ui,
        searched: usize,
        total: usize,
        found: usize,
        _current: i64,
    ) {
        Frame::default()
            .fill(COLOR_BG_DARK)
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                let progress = if total > 0 {
                    searched as f32 / total as f32
                } else {
                    0.0
                };
                ui.add(egui::ProgressBar::new(progress).fill(COLOR_ACCENT));

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} / {}", searched, total))
                            .size(12.0)
                            .color(COLOR_TEXT_DIM),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("已找到 {} 个", found))
                                .size(12.0)
                                .color(COLOR_SUCCESS),
                        );
                    });
                });
            });
        ui.add_space(12.0);
    }

    fn draw_input_section(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("搜索范围")
                .size(13.0)
                .color(COLOR_ACCENT)
                .strong(),
        );
        ui.add_space(8.0);

        egui::Grid::new("input_grid")
            .num_columns(2)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                ui.label(RichText::new("起始种子").size(12.0).color(COLOR_TEXT_DIM));
                ui.add(
                    egui::TextEdit::singleline(&mut self.controls.seed_start).desired_width(140.0),
                );

                ui.label(RichText::new("结束种子").size(12.0).color(COLOR_TEXT_DIM));
                ui.add(
                    egui::TextEdit::singleline(&mut self.controls.seed_end).desired_width(140.0),
                );

                ui.label(RichText::new("版本").size(12.0).color(COLOR_TEXT_DIM));
                ui.add(egui::TextEdit::singleline(&mut self.controls.version).desired_width(140.0));

                ui.label(RichText::new("结果上限").size(12.0).color(COLOR_TEXT_DIM));
                ui.add(egui::TextEdit::singleline(&mut self.controls.limit).desired_width(140.0));
            });
        ui.add_space(12.0);
    }

    fn draw_filter_section(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("筛选条件")
                .size(13.0)
                .color(COLOR_ACCENT)
                .strong(),
        );
        ui.add_space(8.0);

        ui.label(
            RichText::new("需要生物群系")
                .size(11.0)
                .color(COLOR_TEXT_DIM),
        );
        multiselect_dropdown(
            ui,
            &mut self.controls.require_biome,
            BIOME_FILTER_OPTIONS,
            "biome_req",
        );

        ui.add_space(6.0);
        ui.label(
            RichText::new("排除生物群系")
                .size(11.0)
                .color(COLOR_TEXT_DIM),
        );
        multiselect_dropdown(
            ui,
            &mut self.controls.forbid_biome,
            BIOME_FILTER_OPTIONS,
            "biome_forbid",
        );

        ui.add_space(6.0);
        ui.label(RichText::new("需要结构").size(11.0).color(COLOR_TEXT_DIM));
        multiselect_dropdown(
            ui,
            &mut self.controls.require_structure,
            STRUCTURE_FILTER_OPTIONS,
            "struct_req",
        );

        ui.add_space(8.0);
        egui::Grid::new("radius_grid")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label(RichText::new("群系半径").size(11.0).color(COLOR_TEXT_DIM));
                ui.add(
                    egui::TextEdit::singleline(&mut self.controls.biome_radius).desired_width(80.0),
                );

                ui.label(RichText::new("结构半径").size(11.0).color(COLOR_TEXT_DIM));
                ui.add(
                    egui::TextEdit::singleline(&mut self.controls.structure_radius)
                        .desired_width(80.0),
                );

                ui.label(RichText::new("最大高差").size(11.0).color(COLOR_TEXT_DIM));
                ui.add(
                    egui::TextEdit::singleline(&mut self.controls.max_relief).desired_width(80.0),
                );
            });
    }

    fn draw_action_buttons(&mut self, ui: &mut egui::Ui, ctx: &EguiContext) {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            let search_text = if self.is_searching {
                "搜索中..."
            } else {
                "开始搜索"
            };
            let mut search_btn =
                Button::new(RichText::new(search_text).size(14.0).color(COLOR_BG_DARK));
            if !self.is_searching {
                search_btn = search_btn.fill(COLOR_ACCENT);
            }
            if ui.add_sized([140.0, 36.0], search_btn).clicked() && !self.is_searching {
                self.start_search(ctx);
            }

            if self.is_searching && ui.button("停止").clicked() {
                self.stop_search();
            }
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("清空结果").clicked() {
                self.results.clear();
                self.selected_result = None;
                self.preview_texture = None;
            }

            if !self.results.is_empty() && ui.button("导出").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Text", &["txt"])
                    .save_file()
                {
                    let _ = fs::write(path, self.export_results());
                }
            }
        });
    }

    fn draw_templates_panel(&mut self, ui: &mut egui::Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            Frame::default()
                .inner_margin(Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    let templates = get_templates_by_category();
                    for (category, items) in templates {
                        ui.label(
                            RichText::new(format!("{} {}", category.icon(), category.display()))
                                .size(14.0)
                                .color(COLOR_ACCENT)
                                .strong(),
                        );
                        ui.add_space(6.0);

                        for template in items {
                            let is_selected =
                                self.selected_template.as_deref() == Some(template.id);
                            let frame = Frame::default()
                                .fill(if is_selected {
                                    COLOR_ACCENT_DIM
                                } else {
                                    COLOR_BG_DARK
                                })
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(Margin::same(10));

                            frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(
                                            RichText::new(template.name)
                                                .size(13.0)
                                                .color(COLOR_TEXT)
                                                .strong(),
                                        );
                                        ui.label(
                                            RichText::new(template.description)
                                                .size(11.0)
                                                .color(COLOR_TEXT_DIM),
                                        );
                                    });
                                });
                            });

                            if ui
                                .interact(ui.max_rect(), ui.next_auto_id(), egui::Sense::click())
                                .clicked()
                            {
                                self.apply_template(&template);
                            }
                            ui.add_space(4.0);
                        }
                        ui.add_space(12.0);
                    }
                });
        });
    }

    fn draw_favorites_panel(&mut self, ui: &mut egui::Ui, _ctx: &EguiContext) {
        ScrollArea::vertical().show(ui, |ui| {
            Frame::default()
                .inner_margin(Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    if self.favorites.is_empty() {
                        ui.label(RichText::new("暂无收藏").size(14.0).color(COLOR_TEXT_DIM));
                    } else {
                        for (i, fav) in self.favorites.iter().enumerate() {
                            if ui
                                .button(format!(
                                    "Seed {} - {}分",
                                    fav.seed,
                                    fav.score.as_ref().map(|s| s.grade.display()).unwrap_or("?")
                                ))
                                .clicked()
                            {
                                self.results = self.favorites.clone();
                                self.selected_result = Some(i);
                                self.current_tab = AppTab::Search;
                            }
                        }
                    }
                });
        });
    }

    fn draw_content(&mut self, ui: &mut egui::Ui, ctx: &EguiContext) {
        egui::CentralPanel::default()
            .frame(Frame::default().fill(COLOR_BG_DARK))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    self.draw_preview_panel(ui);
                    self.draw_results_panel(ui, ctx);
                });
            });
    }

    fn draw_preview_panel(&mut self, ui: &mut egui::Ui) {
        Frame::default()
            .fill(COLOR_BG_CARD)
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::same(16))
            .show(ui, |ui| {
                ui.set_min_size(Vec2::new(400.0, ui.available_height()));

                ui.label(
                    RichText::new("地图预览")
                        .size(16.0)
                        .color(COLOR_TEXT)
                        .strong(),
                );
                ui.add_space(12.0);

                if let Some(texture) = &self.preview_texture {
                    let size = texture.size_vec2();
                    let scale = (ui.available_width() / size.x)
                        .min(ui.available_height() / size.y)
                        .min(1.0);
                    ui.image((texture.id(), size * scale));
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new("选择种子查看预览")
                                .size(14.0)
                                .color(COLOR_TEXT_DIM),
                        );
                    });
                }

                if let Some(idx) = self.selected_result {
                    self.draw_seed_details(ui, &self.results[idx].clone());
                }
            });
    }

    fn draw_seed_details(&self, ui: &mut egui::Ui, summary: &MatchSummary) {
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);

        ui.label(
            RichText::new(format!("Seed: {}", summary.seed))
                .size(18.0)
                .color(COLOR_ACCENT)
                .strong(),
        );

        if let Some(score) = &summary.score {
            ui.add_space(8.0);
            let (r, g, b) = score.grade.color();
            Frame::default()
                .fill(Color32::from_rgb(r, g, b))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(12, 6))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} ({:.0}分)",
                            score.grade.display(),
                            score.overall
                        ))
                        .size(14.0)
                        .color(Color32::BLACK)
                        .strong(),
                    );
                });

            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                for (label, val) in [
                    ("资源", score.resource_score),
                    ("安全", score.safety_score),
                    ("群系", score.biome_diversity),
                    ("结构", score.structure_score),
                ] {
                    ui.label(
                        RichText::new(format!("{}:{:.0}", label, val))
                            .size(11.0)
                            .color(COLOR_TEXT_DIM),
                    );
                }
            });
        }

        ui.add_space(8.0);
        ui.label(
            RichText::new(format!(
                "出生点: ({}, {})",
                summary.spawn.x, summary.spawn.z
            ))
            .size(12.0)
            .color(COLOR_TEXT),
        );
        ui.label(
            RichText::new(format_nether_info(summary.spawn))
                .size(11.0)
                .color(COLOR_TEXT_DIM),
        );
        ui.label(
            RichText::new(format!(
                "TP: {}",
                format_teleport_command(summary.spawn, "overworld")
            ))
            .size(10.0)
            .color(COLOR_TEXT_DIM),
        );

        if let Some(analysis) = &summary.spawn_analysis {
            ui.add_space(8.0);
            if analysis.is_safe {
                ui.label(
                    RichText::new("✓ 安全出生点")
                        .size(12.0)
                        .color(COLOR_SUCCESS),
                );
            } else {
                ui.label(
                    RichText::new("⚠ 出生点有风险")
                        .size(12.0)
                        .color(COLOR_WARNING),
                );
            }
        }

        if !summary.structures.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("结构:").size(12.0).color(COLOR_TEXT_DIM));
            for (s, hit) in &summary.structures {
                ui.label(
                    RichText::new(format!("  {} @ {:.0}m", s, hit.distance))
                        .size(11.0)
                        .color(COLOR_TEXT),
                );
            }
        }
    }

    fn draw_results_panel(&mut self, ui: &mut egui::Ui, ctx: &EguiContext) {
        let mut clicked_idx: Option<usize> = None;

        Frame::default()
            .fill(COLOR_BG_CARD)
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::same(16))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("搜索结果")
                            .size(16.0)
                            .color(COLOR_TEXT)
                            .strong(),
                    );
                    ui.label(
                        RichText::new(format!("({} 个)", self.results.len()))
                            .size(12.0)
                            .color(COLOR_TEXT_DIM),
                    );
                });
                ui.add_space(12.0);

                ScrollArea::vertical().show(ui, |ui| {
                    for (idx, result) in self.results.iter().enumerate() {
                        let selected = self.selected_result == Some(idx);
                        let bg = if selected {
                            COLOR_ACCENT_DIM
                        } else {
                            COLOR_BG_DARK
                        };

                        let response = Frame::default()
                            .fill(bg)
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(Margin::same(12))
                            .stroke(Stroke::new(
                                1.0,
                                if selected { COLOR_ACCENT } else { COLOR_BORDER },
                            ))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    if let Some(score) = &result.score {
                                        let (r, g, b) = score.grade.color();
                                        Frame::default()
                                            .fill(Color32::from_rgb(r, g, b))
                                            .corner_radius(CornerRadius::same(4))
                                            .inner_margin(Margin::symmetric(8, 4))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new(score.grade.display())
                                                        .size(12.0)
                                                        .color(Color32::BLACK)
                                                        .strong(),
                                                );
                                            });
                                    }

                                    ui.label(
                                        RichText::new(format!("{}", result.seed))
                                            .size(15.0)
                                            .color(COLOR_TEXT)
                                            .strong(),
                                    );

                                    if result.is_favorite {
                                        ui.label(RichText::new("★").size(14.0).color(COLOR_ACCENT));
                                    }
                                });

                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(format!(
                                        "({}, {})",
                                        result.spawn.x, result.spawn.z
                                    ))
                                    .size(11.0)
                                    .color(COLOR_TEXT_DIM),
                                );

                                if !result.structures.is_empty() {
                                    let structs: Vec<_> = result
                                        .structures
                                        .iter()
                                        .map(|(s, h)| format!("{} {:.0}m", s, h.distance))
                                        .take(3)
                                        .collect();
                                    ui.label(
                                        RichText::new(structs.join(" | "))
                                            .size(10.0)
                                            .color(COLOR_TEXT_DIM),
                                    );
                                }
                            })
                            .response;

                        if response.clicked() {
                            clicked_idx = Some(idx);
                        }

                        ui.add_space(4.0);
                    }
                });
            });

        if let Some(idx) = clicked_idx {
            self.select_result(idx, ctx);
        }
    }
}

fn multiselect_dropdown(ui: &mut egui::Ui, value: &mut String, options: &[FilterOption], id: &str) {
    let selected: BTreeSet<String> = value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    let summary = if selected.is_empty() {
        "未选择".into()
    } else {
        options
            .iter()
            .filter(|o| selected.contains(o.key))
            .map(|o| o.zh)
            .take(2)
            .collect::<Vec<_>>()
            .join(", ")
    };

    egui::ComboBox::from_id_salt(id)
        .selected_text(summary)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for option in options {
                let mut checked = selected.contains(option.key);
                if ui
                    .checkbox(&mut checked, format!("{} ({})", option.zh, option.key))
                    .changed()
                {
                    let mut new_set = selected.clone();
                    if checked {
                        new_set.insert(option.key.to_string());
                    } else {
                        new_set.remove(option.key);
                    }
                    *value = new_set.into_iter().collect::<Vec<_>>().join(",");
                }
            }
        });
}

fn configure_fonts(ctx: &EguiContext) {
    let mut fonts = FontDefinitions::default();

    let font_paths = if cfg!(target_os = "macos") {
        vec![
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
        ]
    } else if cfg!(target_os = "windows") {
        vec![
            "C:\\Windows\\Fonts\\msyh.ttc",
            "C:\\Windows\\Fonts\\simhei.ttf",
        ]
    } else {
        vec!["/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"]
    };

    for path in font_paths {
        if let Ok(bytes) = fs::read(path) {
            let font_name: String = "cjk-font".into();
            fonts
                .font_data
                .insert(font_name.clone(), FontData::from_owned(bytes).into());
            fonts
                .families
                .get_mut(&FontFamily::Proportional)
                .unwrap()
                .insert(0, font_name);
            break;
        }
    }

    ctx.set_fonts(fonts);
}

fn configure_style(ctx: &EguiContext) {
    let mut style = (*ctx.style()).clone();

    style.visuals = egui::Visuals::dark();
    style.visuals.override_text_color = Some(COLOR_TEXT);
    style.visuals.panel_fill = COLOR_BG_DARK;
    style.visuals.widgets.inactive.bg_fill = COLOR_BG_CARD;
    style.visuals.widgets.hovered.bg_fill = COLOR_BG_HOVER;
    style.visuals.widgets.active.bg_fill = COLOR_ACCENT_DIM;
    style.visuals.selection.bg_fill = COLOR_ACCENT;
    style.visuals.window_corner_radius = CornerRadius::same(12);

    ctx.set_style(style);
}
