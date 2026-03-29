use crate::config::DEFAULT_VERSION_INDEX;

#[derive(Debug, Clone)]
pub struct SearchTemplate {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub category: TemplateCategory,
    pub config: TemplateConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemplateCategory {
    Survival,
    Building,
    Speedrun,
    Farming,
    Exploration,
    Hardcore,
    Multiplayer,
}

impl TemplateCategory {
    pub fn display(&self) -> &'static str {
        match self {
            TemplateCategory::Survival => "生存",
            TemplateCategory::Building => "建筑",
            TemplateCategory::Speedrun => "速通",
            TemplateCategory::Farming => "农场",
            TemplateCategory::Exploration => "探险",
            TemplateCategory::Hardcore => "硬核",
            TemplateCategory::Multiplayer => "多人",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            TemplateCategory::Survival => "🌲",
            TemplateCategory::Building => "🏗",
            TemplateCategory::Speedrun => "⚡",
            TemplateCategory::Farming => "🌾",
            TemplateCategory::Exploration => "🗺",
            TemplateCategory::Hardcore => "💀",
            TemplateCategory::Multiplayer => "👥",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TemplateConfig {
    pub seed_start: String,
    pub seed_end: String,
    pub version_index: usize,
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
}

impl Default for TemplateConfig {
    fn default() -> Self {
        Self {
            seed_start: "1".into(),
            seed_end: "1000000".into(),
            version_index: DEFAULT_VERSION_INDEX,
            limit: "30".into(),
            require_biome: String::new(),
            forbid_biome: String::new(),
            biome_radius: "256".into(),
            biome_step: "16".into(),
            require_structure: String::new(),
            structure_radius: "1000".into(),
            terrain_radius: "160".into(),
            min_avg_height: String::new(),
            max_avg_height: String::new(),
            max_relief: String::new(),
        }
    }
}

pub fn get_all_templates() -> Vec<SearchTemplate> {
    vec![
        // ==================== 生存模板 ====================
        SearchTemplate {
            id: "survival_balanced",
            name: "均衡生存",
            description: "资源丰富、安全出生点、多种群系，适合长期生存",
            category: TemplateCategory::Survival,
            config: TemplateConfig {
                require_biome: "plains,forest,river".into(),
                require_structure: "village".into(),
                structure_radius: "800".into(),
                max_relief: "50".into(),
                ..Default::default()
            },
        },
        SearchTemplate {
            id: "survival_hard",
            name: "硬核生存",
            description: "远离危险区域，优先安全开局",
            category: TemplateCategory::Survival,
            config: TemplateConfig {
                require_biome: "plains,forest".into(),
                forbid_biome: "ocean,deep_ocean".into(),
                require_structure: "village".into(),
                structure_radius: "600".into(),
                max_relief: "40".into(),
                ..Default::default()
            },
        },
        SearchTemplate {
            id: "survival_island",
            name: "岛屿生存",
            description: "出生在小岛上，挑战模式",
            category: TemplateCategory::Survival,
            config: TemplateConfig {
                require_biome: "ocean,beach".into(),
                biome_radius: "400".into(),
                ..Default::default()
            },
        },
        // ==================== 建筑模板 ====================
        SearchTemplate {
            id: "building_scenic",
            name: "风景建筑",
            description: "壮观地形背景：山脉、峡谷、瀑布",
            category: TemplateCategory::Building,
            config: TemplateConfig {
                require_biome: "meadow,stony_peaks,river".into(),
                biome_radius: "512".into(),
                max_relief: "60".into(),
                ..Default::default()
            },
        },
        SearchTemplate {
            id: "building_flat",
            name: "平坦建造",
            description: "大面积平坦区域，适合大型建筑项目",
            category: TemplateCategory::Building,
            config: TemplateConfig {
                require_biome: "plains".into(),
                biome_radius: "512".into(),
                max_relief: "25".into(),
                ..Default::default()
            },
        },
        SearchTemplate {
            id: "building_cherry",
            name: "樱花胜地",
            description: "樱花林+雪山背景，绝佳建筑选址",
            category: TemplateCategory::Building,
            config: TemplateConfig {
                require_biome: "cherry_grove,snowy_slopes".into(),
                biome_radius: "512".into(),
                ..Default::default()
            },
        },
        SearchTemplate {
            id: "building_mountain",
            name: "山顶豪宅",
            description: "高山地形，适合悬空建筑",
            category: TemplateCategory::Building,
            config: TemplateConfig {
                require_biome: "stony_peaks,jagged_peaks".into(),
                biome_radius: "400".into(),
                min_avg_height: "90".into(),
                ..Default::default()
            },
        },
        // ==================== 速通模板 ====================
        SearchTemplate {
            id: "speedrun_village",
            name: "村庄开局",
            description: "近距离村庄+食物资源，快速过渡",
            category: TemplateCategory::Speedrun,
            config: TemplateConfig {
                require_biome: "plains".into(),
                require_structure: "village,shipwreck".into(),
                structure_radius: "500".into(),
                ..Default::default()
            },
        },
        SearchTemplate {
            id: "speedrun_classic",
            name: "经典速通",
            description: "村庄+沉船+埋藏宝藏，资源最大化",
            category: TemplateCategory::Speedrun,
            config: TemplateConfig {
                require_structure: "village,shipwreck,treasure".into(),
                structure_radius: "600".into(),
                ..Default::default()
            },
        },
        // ==================== 农场模板 ====================
        SearchTemplate {
            id: "farm_iron",
            name: "刷铁机",
            description: "多个村庄近距离分布，适合建造刷铁机",
            category: TemplateCategory::Farming,
            config: TemplateConfig {
                require_structure: "village".into(),
                structure_radius: "1000".into(),
                limit: "50".into(),
                ..Default::default()
            },
        },
        SearchTemplate {
            id: "farm_guardian",
            name: "守卫者农场",
            description: "近距海底神殿，经验/海晶农场",
            category: TemplateCategory::Farming,
            config: TemplateConfig {
                require_structure: "monument".into(),
                structure_radius: "1500".into(),
                ..Default::default()
            },
        },
        SearchTemplate {
            id: "farm_slime",
            name: "史莱姆农场",
            description: "沼泽区域，适合史莱姆农场",
            category: TemplateCategory::Farming,
            config: TemplateConfig {
                require_biome: "swamp".into(),
                biome_radius: "256".into(),
                ..Default::default()
            },
        },
        SearchTemplate {
            id: "farm_mushroom",
            name: "蘑菇岛刷怪塔",
            description: "蘑菇岛，最佳刷怪塔选址",
            category: TemplateCategory::Farming,
            config: TemplateConfig {
                require_biome: "mushroom_fields".into(),
                biome_radius: "512".into(),
                ..Default::default()
            },
        },
        SearchTemplate {
            id: "farm_gold",
            name: "金农场",
            description: "下界要塞+堡垒，猪灵交易农场",
            category: TemplateCategory::Farming,
            config: TemplateConfig {
                require_structure: "fortress,bastion".into(),
                structure_radius: "2000".into(),
                ..Default::default()
            },
        },
        // ==================== 探险模板 ====================
        SearchTemplate {
            id: "explore_biomes",
            name: "群系收集者",
            description: "多种群系近距离分布",
            category: TemplateCategory::Exploration,
            config: TemplateConfig {
                require_biome: "plains,forest,desert,jungle,swamp".into(),
                biome_radius: "512".into(),
                limit: "50".into(),
                ..Default::default()
            },
        },
        SearchTemplate {
            id: "explore_rare",
            name: "稀有群系猎手",
            description: "樱花林、蘑菇岛、深暗之域等稀有群系",
            category: TemplateCategory::Exploration,
            config: TemplateConfig {
                require_biome: "cherry_grove,mushroom_fields".into(),
                biome_radius: "800".into(),
                limit: "50".into(),
                ..Default::default()
            },
        },
        SearchTemplate {
            id: "explore_structures",
            name: "结构探索者",
            description: "多种结构近距离分布",
            category: TemplateCategory::Exploration,
            config: TemplateConfig {
                require_structure: "village,desert_pyramid,jungle_temple,monument".into(),
                structure_radius: "2000".into(),
                limit: "30".into(),
                ..Default::default()
            },
        },
        SearchTemplate {
            id: "explore_ancient",
            name: "远古城市",
            description: "寻找深暗之域和远古城市",
            category: TemplateCategory::Exploration,
            config: TemplateConfig {
                require_biome: "deep_dark".into(),
                require_structure: "ancient_city".into(),
                biome_radius: "400".into(),
                structure_radius: "1500".into(),
                ..Default::default()
            },
        },
        // ==================== 硬核模板 ====================
        SearchTemplate {
            id: "hardcore_desert",
            name: "沙漠求生",
            description: "极端沙漠环境，资源匮乏",
            category: TemplateCategory::Hardcore,
            config: TemplateConfig {
                require_biome: "desert".into(),
                forbid_biome: "ocean,river".into(),
                biome_radius: "400".into(),
                ..Default::default()
            },
        },
        SearchTemplate {
            id: "hardcore_frozen",
            name: "冰封世界",
            description: "雪原开局，食物极度稀缺",
            category: TemplateCategory::Hardcore,
            config: TemplateConfig {
                require_biome: "snowy_tundra,snowy_taiga".into(),
                biome_radius: "400".into(),
                ..Default::default()
            },
        },
        // ==================== 多人模板 ====================
        SearchTemplate {
            id: "multi_spawn",
            name: "多人出生点",
            description: "大面积平坦区域+资源均衡",
            category: TemplateCategory::Multiplayer,
            config: TemplateConfig {
                require_biome: "plains,forest".into(),
                forbid_biome: "ocean".into(),
                biome_radius: "512".into(),
                max_relief: "35".into(),
                require_structure: "village".into(),
                structure_radius: "1000".into(),
                ..Default::default()
            },
        },
        SearchTemplate {
            id: "multi_anti_cheat",
            name: "刷怪塔服务区",
            description: "寻找刷怪塔选址，保障服务区运营",
            category: TemplateCategory::Multiplayer,
            config: TemplateConfig {
                require_biome: "mushroom_fields".into(),
                biome_radius: "256".into(),
                forbid_biome: "ocean".into(),
                max_relief: "30".into(),
                ..Default::default()
            },
        },
    ]
}

pub fn get_templates_by_category() -> Vec<(TemplateCategory, Vec<SearchTemplate>)> {
    let templates = get_all_templates();
    let mut grouped: std::collections::HashMap<TemplateCategory, Vec<SearchTemplate>> =
        std::collections::HashMap::new();

    for template in templates.into_iter() {
        grouped.entry(template.category).or_default().push(template);
    }

    let mut result: Vec<_> = grouped.into_iter().collect();
    result.sort_by_key(|(cat, _)| *cat as u8);
    result
}

pub fn get_template_by_id(id: &str) -> Option<SearchTemplate> {
    get_all_templates().into_iter().find(|t| t.id == id)
}
