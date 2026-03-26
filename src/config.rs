use anyhow::{ensure, Result};
use cubiomes::enums::{BiomeID, MCVersion, StructureType};

use crate::parser::{
    parse_biome_csv, parse_i32, parse_i64, parse_optional_f32, parse_structure_csv, parse_usize,
    parse_version_info,
};

#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub seed_start: i64,
    pub seed_end: i64,
    pub version: MCVersion,
    pub version_label: String,
    pub version_warning: Option<String>,
    pub limit: usize,
    pub required_biomes: Vec<BiomeID>,
    pub forbidden_biomes: Vec<BiomeID>,
    pub biome_radius: i32,
    pub biome_step: i32,
    pub required_structures: Vec<StructureType>,
    pub structure_radius: i32,
    pub terrain_radius: i32,
    pub min_avg_height: Option<f32>,
    pub max_avg_height: Option<f32>,
    pub max_relief: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct Controls {
    pub seed_start: String,
    pub seed_end: String,
    pub version: String,
    pub limit: String,
    pub require_biome: String,
    pub forbid_biome: String,
    pub biome_radius: String,
    pub biome_step: String,
    pub require_structure: String,
    pub structure_radius: String,
    pub terrain_radius: String,
    pub min_avg_height: String,
    pub max_avg_height: String,
    pub max_relief: String,
    pub preview_radius: String,
    pub preview_image_size: String,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            seed_start: "1".into(),
            seed_end: "500000".into(),
            version: "1.21.1".into(),
            limit: "20".into(),
            require_biome: "plains,cherry_grove".into(),
            forbid_biome: String::new(),
            biome_radius: "256".into(),
            biome_step: "16".into(),
            require_structure: "village".into(),
            structure_radius: "800".into(),
            terrain_radius: "160".into(),
            min_avg_height: String::new(),
            max_avg_height: String::new(),
            max_relief: "45".into(),
            preview_radius: "512".into(),
            preview_image_size: "384".into(),
        }
    }
}

impl Controls {
    pub fn to_config(&self) -> Result<SearchConfig> {
        let seed_start = parse_i64(&self.seed_start, "起始种子")?;
        let seed_end = parse_i64(&self.seed_end, "结束种子")?;
        ensure!(seed_end >= seed_start, "结束种子必须大于或等于起始种子");
        let (version, version_label, version_warning) = parse_version_info(&self.version)?;

        Ok(SearchConfig {
            seed_start,
            seed_end,
            version,
            version_label,
            version_warning,
            limit: parse_usize(&self.limit, "命中上限")?,
            required_biomes: parse_biome_csv(&self.require_biome)?,
            forbidden_biomes: parse_biome_csv(&self.forbid_biome)?,
            biome_radius: parse_i32(&self.biome_radius, "生物群系半径")?,
            biome_step: parse_i32(&self.biome_step, "生物群系采样步长")?,
            required_structures: parse_structure_csv(&self.require_structure)?,
            structure_radius: parse_i32(&self.structure_radius, "结构半径")?,
            terrain_radius: parse_i32(&self.terrain_radius, "地形半径")?,
            min_avg_height: parse_optional_f32(&self.min_avg_height, "最小平均高度")?,
            max_avg_height: parse_optional_f32(&self.max_avg_height, "最大平均高度")?,
            max_relief: parse_optional_f32(&self.max_relief, "最大地形高差")?,
        })
    }

    pub fn preview_radius(&self) -> Result<i32> {
        parse_i32(&self.preview_radius, "预览半径")
    }

    pub fn preview_image_size(&self) -> Result<usize> {
        let size = parse_usize(&self.preview_image_size, "预览尺寸")?;
        ensure!(size >= 128, "预览尺寸至少为 128");
        ensure!(size <= 1024, "预览尺寸建议不超过 1024");
        Ok(size)
    }
}

pub fn total_seeds(config: &SearchConfig) -> usize {
    (config.seed_end - config.seed_start + 1) as usize
}

pub fn search_worker_count(config: &SearchConfig) -> usize {
    let total = total_seeds(config).max(1);
    let available = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1);
    available.min(total)
}
