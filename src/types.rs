use cubiomes::{
    enums::{BiomeID, MCVersion, StructureType},
    generator::BlockPosition,
};
use eframe::egui::ColorImage;

#[derive(Debug, Clone, Copy)]
pub struct TerrainStats {
    pub min_height: f32,
    pub max_height: f32,
    pub avg_height: f32,
    pub relief: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct LocatedBiome {
    pub position: BlockPosition,
    pub distance: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct LocatedStructure {
    pub position: BlockPosition,
    pub distance: f64,
}

#[derive(Debug, Clone)]
pub struct MatchSummary {
    pub seed: i64,
    pub version: MCVersion,
    pub version_label: String,
    pub version_warning: Option<String>,
    pub spawn: BlockPosition,
    pub terrain: Option<TerrainStats>,
    pub biomes: Vec<(BiomeID, LocatedBiome)>,
    pub structures: Vec<(StructureType, LocatedStructure)>,
    pub is_favorite: bool,
}

impl Default for MatchSummary {
    fn default() -> Self {
        Self {
            seed: 0,
            version: MCVersion::MC_1_21,
            version_label: String::new(),
            version_warning: None,
            spawn: BlockPosition::new(0, 0),
            terrain: None,
            biomes: Vec::new(),
            structures: Vec::new(),
            is_favorite: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreviewRequest {
    pub token: u64,
    pub summary: MatchSummary,
    pub radius: i32,
    pub image_size: usize,
}

#[derive(Debug)]
pub struct PreviewResponse {
    pub token: u64,
    pub image: Result<ColorImage, String>,
}

#[derive(Debug)]
pub enum WorkerMessage {
    Progress {
        searched: usize,
        total: usize,
        found: usize,
        current_seed: i64,
    },
    Match(MatchSummary),
    Finished {
        searched: usize,
        found: usize,
        stopped: bool,
    },
    Error(String),
}

pub enum SearchEvent {
    Match(MatchSummary),
    Error(String),
    WorkerDone,
}

#[derive(Clone, Copy)]
pub struct FilterOption {
    pub key: &'static str,
    pub zh: &'static str,
}

pub const BIOME_FILTER_OPTIONS: &[FilterOption] = &[
    FilterOption {
        key: "plains",
        zh: "平原",
    },
    FilterOption {
        key: "forest",
        zh: "森林",
    },
    FilterOption {
        key: "flower_forest",
        zh: "繁花森林",
    },
    FilterOption {
        key: "birch_forest",
        zh: "白桦森林",
    },
    FilterOption {
        key: "dark_forest",
        zh: "黑森林",
    },
    FilterOption {
        key: "taiga",
        zh: "针叶林",
    },
    FilterOption {
        key: "snowy_taiga",
        zh: "积雪针叶林",
    },
    FilterOption {
        key: "jungle",
        zh: "丛林",
    },
    FilterOption {
        key: "bamboo_jungle",
        zh: "竹林",
    },
    FilterOption {
        key: "savanna",
        zh: "热带草原",
    },
    FilterOption {
        key: "desert",
        zh: "沙漠",
    },
    FilterOption {
        key: "swamp",
        zh: "沼泽",
    },
    FilterOption {
        key: "mangrove_swamp",
        zh: "红树林沼泽",
    },
    FilterOption {
        key: "meadow",
        zh: "草甸",
    },
    FilterOption {
        key: "grove",
        zh: "雪林",
    },
    FilterOption {
        key: "cherry_grove",
        zh: "樱花树林",
    },
    FilterOption {
        key: "stony_peaks",
        zh: "裸岩山峰",
    },
    FilterOption {
        key: "jagged_peaks",
        zh: "尖峭山峰",
    },
    FilterOption {
        key: "frozen_peaks",
        zh: "冰封山峰",
    },
    FilterOption {
        key: "snowy_slopes",
        zh: "积雪山坡",
    },
    FilterOption {
        key: "lush_caves",
        zh: "繁茂洞穴",
    },
    FilterOption {
        key: "dripstone_caves",
        zh: "溶洞",
    },
    FilterOption {
        key: "deep_dark",
        zh: "深暗之域",
    },
    FilterOption {
        key: "badlands",
        zh: "恶地",
    },
    FilterOption {
        key: "eroded_badlands",
        zh: "风蚀恶地",
    },
    FilterOption {
        key: "mushroom_fields",
        zh: "蘑菇岛",
    },
    FilterOption {
        key: "beach",
        zh: "海滩",
    },
    FilterOption {
        key: "stone_shore",
        zh: "石岸",
    },
    FilterOption {
        key: "river",
        zh: "河流",
    },
    FilterOption {
        key: "ocean",
        zh: "海洋",
    },
];

pub const STRUCTURE_FILTER_OPTIONS: &[FilterOption] = &[
    FilterOption {
        key: "village",
        zh: "村庄",
    },
    FilterOption {
        key: "shipwreck",
        zh: "沉船",
    },
    FilterOption {
        key: "ocean_ruin",
        zh: "海底废墟",
    },
    FilterOption {
        key: "treasure",
        zh: "埋藏的宝藏",
    },
    FilterOption {
        key: "mineshaft",
        zh: "废弃矿井",
    },
    FilterOption {
        key: "outpost",
        zh: "掠夺者前哨站",
    },
    FilterOption {
        key: "mansion",
        zh: "林地府邸",
    },
    FilterOption {
        key: "monument",
        zh: "海底神殿",
    },
    FilterOption {
        key: "desert_pyramid",
        zh: "沙漠神殿",
    },
    FilterOption {
        key: "jungle_temple",
        zh: "丛林神庙",
    },
    FilterOption {
        key: "swamp_hut",
        zh: "沼泽小屋",
    },
    FilterOption {
        key: "igloo",
        zh: "雪屋",
    },
    FilterOption {
        key: "ruined_portal",
        zh: "废弃传送门",
    },
    FilterOption {
        key: "ancient_city",
        zh: "远古城市",
    },
    FilterOption {
        key: "geode",
        zh: "紫晶洞",
    },
    FilterOption {
        key: "fortress",
        zh: "下界要塞",
    },
    FilterOption {
        key: "bastion",
        zh: "堡垒遗迹",
    },
    FilterOption {
        key: "end_city",
        zh: "末地城",
    },
    FilterOption {
        key: "trail_ruins",
        zh: "古迹废墟",
    },
    FilterOption {
        key: "trial_chambers",
        zh: "试炼密室",
    },
];

pub fn block_distance(a: BlockPosition, b: BlockPosition) -> f64 {
    let dx = (a.x - b.x) as f64;
    let dz = (a.z - b.z) as f64;
    (dx * dx + dz * dz).sqrt()
}
