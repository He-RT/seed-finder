use std::collections::HashSet;

use cubiomes::{
    enums::{BiomeID, StructureType},
    generator::BlockPosition,
};

use crate::types::{LocatedBiome, LocatedStructure, MatchSummary, TerrainStats};

#[derive(Debug, Clone)]
pub struct SeedScore {
    pub overall: f32,
    pub resource_score: f32,
    pub biome_diversity: f32,
    pub structure_score: f32,
    pub terrain_score: f32,
    pub safety_score: f32,
    pub farming_score: f32,
    pub exploration_score: f32,
    pub grade: SeedGrade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedGrade {
    S,
    A,
    B,
    C,
    D,
    F,
}

impl SeedGrade {
    pub fn from_score(score: f32) -> Self {
        if score >= 90.0 {
            SeedGrade::S
        } else if score >= 75.0 {
            SeedGrade::A
        } else if score >= 60.0 {
            SeedGrade::B
        } else if score >= 45.0 {
            SeedGrade::C
        } else if score >= 30.0 {
            SeedGrade::D
        } else {
            SeedGrade::F
        }
    }

    pub fn display(&self) -> &'static str {
        match self {
            SeedGrade::S => "S",
            SeedGrade::A => "A",
            SeedGrade::B => "B",
            SeedGrade::C => "C",
            SeedGrade::D => "D",
            SeedGrade::F => "F",
        }
    }

    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            SeedGrade::S => (255, 215, 0),
            SeedGrade::A => (0, 255, 127),
            SeedGrade::B => (100, 200, 255),
            SeedGrade::C => (200, 200, 200),
            SeedGrade::D => (255, 165, 0),
            SeedGrade::F => (255, 80, 80),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SpawnAnalysis {
    pub is_safe: bool,
    pub has_land_nearby: bool,
    pub water_percentage: f32,
    pub land_distance: Option<f32>,
    pub issues: Vec<String>,
    pub advantages: Vec<String>,
    pub nearby_resources: NearbyResources,
}

#[derive(Debug, Clone, Default)]
pub struct NearbyResources {
    pub trees: bool,
    pub animals: Vec<AnimalType>,
    pub food_sources: Vec<FoodSource>,
    pub caves_nearby: bool,
    pub water_source: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AnimalType {
    Cow,
    Sheep,
    Pig,
    Chicken,
    Horse,
    Llama,
    Wolf,
    Fox,
    Rabbit,
    Panda,
    Parrot,
    Frog,
}

impl AnimalType {
    pub fn display(&self) -> &'static str {
        match self {
            AnimalType::Cow => "牛",
            AnimalType::Sheep => "羊",
            AnimalType::Pig => "猪",
            AnimalType::Chicken => "鸡",
            AnimalType::Horse => "马",
            AnimalType::Llama => "羊驼",
            AnimalType::Wolf => "狼",
            AnimalType::Fox => "狐狸",
            AnimalType::Rabbit => "兔子",
            AnimalType::Panda => "熊猫",
            AnimalType::Parrot => "鹦鹉",
            AnimalType::Frog => "青蛙",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FoodSource {
    Berries,
    Carrots,
    Potatoes,
    Wheat,
    Melon,
    Pumpkin,
    Fish,
    SweetBerries,
}

impl FoodSource {
    pub fn display(&self) -> &'static str {
        match self {
            FoodSource::Berries => "甜浆果",
            FoodSource::Carrots => "胡萝卜",
            FoodSource::Potatoes => "土豆",
            FoodSource::Wheat => "小麦",
            FoodSource::Melon => "西瓜",
            FoodSource::Pumpkin => "南瓜",
            FoodSource::Fish => "鱼",
            FoodSource::SweetBerries => "浆果",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FarmingAnalysis {
    pub iron_golem_villages: Vec<LocatedStructure>,
    pub suitable_mob_tower_locations: Vec<MobTowerLocation>,
    pub special_biomes_for_farms: Vec<(BiomeID, String)>,
}

#[derive(Debug, Clone)]
pub struct MobTowerLocation {
    pub position: BlockPosition,
    pub height_estimate: i32,
    pub surface_type: SurfaceType,
    pub is_ocean: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceType {
    Land,
    Ocean,
    Desert,
    Swamp,
}

#[derive(Debug, Clone)]
pub struct StructureCombo {
    pub structure_type: StructureType,
    pub position: BlockPosition,
    pub distance: f64,
    pub nether_coords: Option<(i32, i32)>,
}

pub fn calculate_seed_score(
    summary: &MatchSummary,
    spawn_analysis: &SpawnAnalysis,
    farming_analysis: &FarmingAnalysis,
) -> SeedScore {
    let resource_score = calculate_resource_score(spawn_analysis, summary);
    let biome_diversity = calculate_biome_diversity_score(summary);
    let structure_score = calculate_structure_score(summary);
    let terrain_score = calculate_terrain_score(&summary.terrain);
    let safety_score = calculate_safety_score(spawn_analysis);
    let farming_score = calculate_farming_score(farming_analysis, summary);
    let exploration_score = calculate_exploration_score(summary);

    let weights = ScoreWeights::default();
    let overall = (resource_score * weights.resource
        + biome_diversity * weights.biome_diversity
        + structure_score * weights.structure
        + terrain_score * weights.terrain
        + safety_score * weights.safety
        + farming_score * weights.farming
        + exploration_score * weights.exploration)
        / weights.total();

    let grade = SeedGrade::from_score(overall);

    SeedScore {
        overall,
        resource_score,
        biome_diversity,
        structure_score,
        terrain_score,
        safety_score,
        farming_score,
        exploration_score,
        grade,
    }
}

#[derive(Clone, Copy)]
struct ScoreWeights {
    resource: f32,
    biome_diversity: f32,
    structure: f32,
    terrain: f32,
    safety: f32,
    farming: f32,
    exploration: f32,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self {
            resource: 1.5,
            biome_diversity: 1.2,
            structure: 1.0,
            terrain: 0.8,
            safety: 1.8,
            farming: 0.7,
            exploration: 1.0,
        }
    }
}

impl ScoreWeights {
    fn total(&self) -> f32 {
        self.resource
            + self.biome_diversity
            + self.structure
            + self.terrain
            + self.safety
            + self.farming
            + self.exploration
    }
}

fn calculate_resource_score(spawn_analysis: &SpawnAnalysis, summary: &MatchSummary) -> f32 {
    let mut score = 50.0;

    if spawn_analysis.nearby_resources.trees {
        score += 15.0;
    }
    if spawn_analysis.nearby_resources.water_source {
        score += 10.0;
    }
    if spawn_analysis.nearby_resources.caves_nearby {
        score += 10.0;
    }

    score += spawn_analysis.nearby_resources.animals.len() as f32 * 5.0;
    score += spawn_analysis.nearby_resources.food_sources.len() as f32 * 3.0;

    for (_, hit) in &summary.biomes {
        if hit.distance < 200.0 {
            score += 3.0;
        }
    }

    score.min(100.0).max(0.0)
}

fn calculate_biome_diversity_score(summary: &MatchSummary) -> f32 {
    let unique_biomes = summary.biomes.len();
    let rare_biomes = count_rare_biomes(summary);

    let diversity_score = (unique_biomes as f32 * 8.0).min(60.0);
    let rare_bonus = (rare_biomes as f32 * 10.0).min(40.0);

    (diversity_score + rare_bonus).min(100.0)
}

fn count_rare_biomes(summary: &MatchSummary) -> usize {
    let rare_biome_ids: HashSet<BiomeID> = [
        BiomeID::mushroom_fields,
        BiomeID::cherry_grove,
        BiomeID::deep_dark,
        BiomeID::bamboo_jungle,
        BiomeID::eroded_badlands,
        BiomeID::ice_spikes,
        BiomeID::pale_garden,
    ]
    .iter()
    .copied()
    .collect();

    summary
        .biomes
        .iter()
        .filter(|(id, _)| rare_biome_ids.contains(id))
        .count()
}

fn calculate_structure_score(summary: &MatchSummary) -> f32 {
    let mut score = 0.0;

    let structure_weights: std::collections::HashMap<StructureType, f32> = [
        (StructureType::Village, 15.0),
        (StructureType::Monument, 12.0),
        (StructureType::Mansion, 12.0),
        (StructureType::Ancient_City, 15.0),
        (StructureType::Trial_Chambers, 12.0),
        (StructureType::Fortress, 10.0),
        (StructureType::Bastion, 10.0),
        (StructureType::Outpost, 8.0),
        (StructureType::Desert_Pyramid, 6.0),
        (StructureType::Jungle_Temple, 6.0),
        (StructureType::Shipwreck, 5.0),
        (StructureType::Ocean_Ruin, 4.0),
        (StructureType::Treasure, 5.0),
        (StructureType::Geode, 4.0),
    ]
    .iter()
    .cloned()
    .collect();

    for (structure, hit) in &summary.structures {
        let base_score = structure_weights.get(structure).copied().unwrap_or(3.0);
        let distance_multiplier = if hit.distance < 500.0 {
            1.2
        } else if hit.distance < 1000.0 {
            1.0
        } else {
            0.7
        };
        score += base_score * distance_multiplier;
    }

    score.min(100.0)
}

fn calculate_terrain_score(terrain: &Option<TerrainStats>) -> f32 {
    let Some(terrain) = terrain else {
        return 50.0;
    };

    let mut score: f32 = 50.0;

    if terrain.avg_height >= 60.0 && terrain.avg_height <= 80.0 {
        score += 15.0;
    } else if terrain.avg_height < 50.0 || terrain.avg_height > 100.0 {
        score -= 10.0;
    }

    if terrain.relief >= 20.0 && terrain.relief <= 45.0 {
        score += 10.0;
    } else if terrain.relief > 60.0 {
        score -= 5.0;
    }

    if terrain.min_height >= 60.0 {
        score += 10.0;
    }

    score.min(100.0).max(0.0)
}

fn calculate_safety_score(spawn_analysis: &SpawnAnalysis) -> f32 {
    let mut score: f32 = 100.0;

    if !spawn_analysis.is_safe {
        score -= 30.0;
    }
    if !spawn_analysis.has_land_nearby {
        score -= 25.0;
    }
    if spawn_analysis.water_percentage > 0.7 {
        score -= 20.0;
    }
    if spawn_analysis.water_percentage > 0.9 {
        score -= 30.0;
    }

    for _ in &spawn_analysis.issues {
        score -= 5.0;
    }

    for _ in &spawn_analysis.advantages {
        score += 3.0;
    }

    score.min(100.0).max(0.0)
}

fn calculate_farming_score(farming_analysis: &FarmingAnalysis, summary: &MatchSummary) -> f32 {
    let mut score = 0.0;

    score += farming_analysis.iron_golem_villages.len() as f32 * 20.0;
    score += farming_analysis.suitable_mob_tower_locations.len() as f32 * 10.0;

    for (_, desc) in &farming_analysis.special_biomes_for_farms {
        match desc.as_str() {
            "蘑菇岛刷怪塔" => score += 25.0,
            "深暗之域守卫者农场" => score += 20.0,
            "沼泽史莱姆农场" => score += 15.0,
            _ => score += 5.0,
        }
    }

    for (_, hit) in &summary.structures {
        if hit.structure_type == StructureType::Monument && hit.distance < 2000.0 {
            score += 15.0;
        }
    }

    score.min(100.0)
}

fn calculate_exploration_score(summary: &MatchSummary) -> f32 {
    let mut score = 30.0;

    let biome_categories = categorize_biomes(&summary.biomes);
    score += biome_categories.len() as f32 * 10.0;

    let structures_in_range = summary
        .structures
        .iter()
        .filter(|(_, hit)| hit.distance < 1500.0)
        .count();
    score += structures_in_range as f32 * 5.0;

    score.min(100.0)
}

fn categorize_biomes(biomes: &[(BiomeID, LocatedBiome)]) -> HashSet<BiomeCategory> {
    let mut categories = HashSet::new();

    for (biome, _) in biomes {
        let category = match biome {
            b if is_forest_biome(*b) => BiomeCategory::Forest,
            b if is_ocean_biome(*b) => BiomeCategory::Ocean,
            b if is_desert_biome(*b) => BiomeCategory::Desert,
            b if is_snow_biome(*b) => BiomeCategory::Snow,
            b if is_jungle_biome(*b) => BiomeCategory::Jungle,
            b if is_mountain_biome(*b) => BiomeCategory::Mountain,
            b if is_swamp_biome(*b) => BiomeCategory::Swamp,
            b if is_cave_biome(*b) => BiomeCategory::Cave,
            _ => BiomeCategory::Other,
        };
        categories.insert(category);
    }

    categories
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BiomeCategory {
    Forest,
    Ocean,
    Desert,
    Snow,
    Jungle,
    Mountain,
    Swamp,
    Cave,
    Other,
}

fn is_forest_biome(biome: BiomeID) -> bool {
    matches!(
        biome,
        BiomeID::forest
            | BiomeID::birch_forest
            | BiomeID::dark_forest
            | BiomeID::flower_forest
            | BiomeID::cherry_grove
            | BiomeID::taiga
            | BiomeID::grove
    )
}

fn is_ocean_biome(biome: BiomeID) -> bool {
    matches!(
        biome,
        BiomeID::ocean
            | BiomeID::deep_ocean
            | BiomeID::warm_ocean
            | BiomeID::cold_ocean
            | BiomeID::frozen_ocean
            | BiomeID::lukewarm_ocean
    )
}

fn is_desert_biome(biome: BiomeID) -> bool {
    matches!(
        biome,
        BiomeID::desert | BiomeID::badlands | BiomeID::eroded_badlands
    )
}

fn is_snow_biome(biome: BiomeID) -> bool {
    matches!(
        biome,
        BiomeID::snowy_tundra
            | BiomeID::snowy_taiga
            | BiomeID::snowy_slopes
            | BiomeID::jagged_peaks
            | BiomeID::frozen_peaks
            | BiomeID::ice_spikes
    )
}

fn is_jungle_biome(biome: BiomeID) -> bool {
    matches!(
        biome,
        BiomeID::jungle | BiomeID::bamboo_jungle | BiomeID::sparse_jungle
    )
}

fn is_mountain_biome(biome: BiomeID) -> bool {
    matches!(
        biome,
        BiomeID::stony_peaks
            | BiomeID::jagged_peaks
            | BiomeID::frozen_peaks
            | BiomeID::meadow
            | BiomeID::windswept_hills
    )
}

fn is_swamp_biome(biome: BiomeID) -> bool {
    matches!(biome, BiomeID::swamp | BiomeID::mangrove_swamp)
}

fn is_cave_biome(biome: BiomeID) -> bool {
    matches!(
        biome,
        BiomeID::dripstone_caves | BiomeID::lush_caves | BiomeID::deep_dark
    )
}

pub fn format_score_report(score: &SeedScore) -> String {
    let mut report = String::new();

    report.push_str(&format!(
        "综合评分: {:.1} ({})\n",
        score.overall,
        score.grade.display()
    ));
    report.push_str(&format!("├─ 资源评分: {:.1}\n", score.resource_score));
    report.push_str(&format!("├─ 安全评分: {:.1}\n", score.safety_score));
    report.push_str(&format!("├─ 群系多样: {:.1}\n", score.biome_diversity));
    report.push_str(&format!("├─ 结构评分: {:.1}\n", score.structure_score));
    report.push_str(&format!("├─ 地形评分: {:.1}\n", score.terrain_score));
    report.push_str(&format!("├─ 农场潜力: {:.1}\n", score.farming_score));
    report.push_str(&format!("└─ 探索价值: {:.1}", score.exploration_score));

    report
}
