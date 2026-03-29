use anyhow::{Context, Result, bail, ensure};
use cubiomes::enums::{BiomeID, StructureType};

pub fn parse_biome_csv(raw: &str) -> Result<Vec<BiomeID>> {
    split_csv(raw)
        .into_iter()
        .map(|item| parse_biome(&item))
        .collect()
}

pub fn parse_structure_csv(raw: &str) -> Result<Vec<StructureType>> {
    split_csv(raw)
        .into_iter()
        .map(|item| parse_structure(&item))
        .collect()
}

pub fn split_csv(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn parse_i64(raw: &str, label: &str) -> Result<i64> {
    raw.trim()
        .parse::<i64>()
        .with_context(|| format!("{label} 不是有效整数"))
}

pub fn parse_i32(raw: &str, label: &str) -> Result<i32> {
    let value = raw
        .trim()
        .parse::<i32>()
        .with_context(|| format!("{label} 不是有效整数"))?;
    ensure!(value > 0, "{label} 必须大于 0");
    Ok(value)
}

pub fn parse_usize(raw: &str, label: &str) -> Result<usize> {
    let value = raw
        .trim()
        .parse::<usize>()
        .with_context(|| format!("{label} 不是有效整数"))?;
    ensure!(value > 0, "{label} 必须大于 0");
    Ok(value)
}

pub fn parse_optional_f32(raw: &str, label: &str) -> Result<Option<f32>> {
    if raw.trim().is_empty() {
        return Ok(None);
    }
    raw.trim()
        .parse::<f32>()
        .map(Some)
        .with_context(|| format!("{label} 不是有效数字"))
}

fn normalize_name(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

pub fn parse_biome(raw: &str) -> Result<BiomeID> {
    let normalized = normalize_name(raw);
    let biome = match normalized.as_str() {
        "ocean" => BiomeID::ocean,
        "plains" => BiomeID::plains,
        "desert" => BiomeID::desert,
        "mountains" | "extreme_hills" => BiomeID::mountains,
        "forest" => BiomeID::forest,
        "taiga" => BiomeID::taiga,
        "swamp" | "swampland" => BiomeID::swamp,
        "river" => BiomeID::river,
        "nether_wastes" | "hell" => BiomeID::nether_wastes,
        "the_end" | "end" | "sky" => BiomeID::the_end,
        "frozen_ocean" => BiomeID::frozen_ocean,
        "frozen_river" => BiomeID::frozen_river,
        "snowy_tundra" | "ice_plains" => BiomeID::snowy_tundra,
        "snowy_mountains" | "ice_mountains" => BiomeID::snowy_mountains,
        "mushroom_fields" | "mushroom_island" => BiomeID::mushroom_fields,
        "mushroom_field_shore" | "mushroom_island_shore" => BiomeID::mushroom_field_shore,
        "beach" => BiomeID::beach,
        "desert_hills" => BiomeID::desert_hills,
        "wooded_hills" | "forest_hills" => BiomeID::wooded_hills,
        "taiga_hills" => BiomeID::taiga_hills,
        "mountain_edge" | "extreme_hills_edge" => BiomeID::mountain_edge,
        "jungle" => BiomeID::jungle,
        "jungle_hills" => BiomeID::jungle_hills,
        "jungle_edge" => BiomeID::jungle_edge,
        "deep_ocean" => BiomeID::deep_ocean,
        "stone_shore" | "stone_beach" | "stony_shore" => BiomeID::stone_shore,
        "snowy_beach" | "cold_beach" => BiomeID::snowy_beach,
        "birch_forest" => BiomeID::birch_forest,
        "birch_forest_hills" => BiomeID::birch_forest_hills,
        "dark_forest" | "roofed_forest" => BiomeID::dark_forest,
        "snowy_taiga" => BiomeID::snowy_taiga,
        "snowy_taiga_hills" => BiomeID::snowy_taiga_hills,
        "giant_tree_taiga" => BiomeID::giant_tree_taiga,
        "giant_tree_taiga_hills" => BiomeID::giant_tree_taiga_hills,
        "wooded_mountains" => BiomeID::wooded_mountains,
        "savanna" => BiomeID::savanna,
        "savanna_plateau" => BiomeID::savanna_plateau,
        "badlands" => BiomeID::badlands,
        "wooded_badlands_plateau" => BiomeID::wooded_badlands_plateau,
        "badlands_plateau" => BiomeID::badlands_plateau,
        "small_end_islands" => BiomeID::small_end_islands,
        "end_midlands" => BiomeID::end_midlands,
        "end_highlands" => BiomeID::end_highlands,
        "end_barrens" => BiomeID::end_barrens,
        "warm_ocean" => BiomeID::warm_ocean,
        "lukewarm_ocean" => BiomeID::lukewarm_ocean,
        "cold_ocean" => BiomeID::cold_ocean,
        "deep_warm_ocean" => BiomeID::deep_warm_ocean,
        "deep_lukewarm_ocean" => BiomeID::deep_lukewarm_ocean,
        "deep_cold_ocean" => BiomeID::deep_cold_ocean,
        "deep_frozen_ocean" => BiomeID::deep_frozen_ocean,
        "seasonal_forest" => BiomeID::seasonal_forest,
        "rainforest" => BiomeID::rainforest,
        "shrubland" => BiomeID::shrubland,
        "the_void" => BiomeID::the_void,
        "sunflower_plains" => BiomeID::sunflower_plains,
        "desert_lakes" => BiomeID::desert_lakes,
        "gravelly_mountains" => BiomeID::gravelly_mountains,
        "flower_forest" => BiomeID::flower_forest,
        "taiga_mountains" => BiomeID::taiga_mountains,
        "swamp_hills" => BiomeID::swamp_hills,
        "ice_spikes" => BiomeID::ice_spikes,
        "modified_jungle" => BiomeID::modified_jungle,
        "modified_jungle_edge" => BiomeID::modified_jungle_edge,
        "tall_birch_forest" => BiomeID::tall_birch_forest,
        "tall_birch_hills" => BiomeID::tall_birch_hills,
        "dark_forest_hills" => BiomeID::dark_forest_hills,
        "snowy_taiga_mountains" => BiomeID::snowy_taiga_mountains,
        "giant_spruce_taiga" => BiomeID::giant_spruce_taiga,
        "giant_spruce_taiga_hills" => BiomeID::giant_spruce_taiga_hills,
        "modified_gravelly_mountains" => BiomeID::modified_gravelly_mountains,
        "shattered_savanna" => BiomeID::shattered_savanna,
        "shattered_savanna_plateau" => BiomeID::shattered_savanna_plateau,
        "eroded_badlands" => BiomeID::eroded_badlands,
        "modified_wooded_badlands_plateau" => BiomeID::modified_wooded_badlands_plateau,
        "modified_badlands_plateau" => BiomeID::modified_badlands_plateau,
        "bamboo_jungle" => BiomeID::bamboo_jungle,
        "bamboo_jungle_hills" => BiomeID::bamboo_jungle_hills,
        "soul_sand_valley" => BiomeID::soul_sand_valley,
        "crimson_forest" => BiomeID::crimson_forest,
        "warped_forest" => BiomeID::warped_forest,
        "basalt_deltas" => BiomeID::basalt_deltas,
        "dripstone_caves" => BiomeID::dripstone_caves,
        "lush_caves" => BiomeID::lush_caves,
        "meadow" => BiomeID::meadow,
        "grove" => BiomeID::grove,
        "snowy_slopes" => BiomeID::snowy_slopes,
        "jagged_peaks" => BiomeID::jagged_peaks,
        "frozen_peaks" => BiomeID::frozen_peaks,
        "stony_peaks" => BiomeID::stony_peaks,
        "deep_dark" => BiomeID::deep_dark,
        "mangrove_swamp" => BiomeID::mangrove_swamp,
        "cherry_grove" => BiomeID::cherry_grove,
        "pale_garden" => BiomeID::pale_garden,
        _ => bail!("不支持的生物群系 '{raw}'"),
    };

    Ok(biome)
}

pub fn parse_structure(raw: &str) -> Result<StructureType> {
    let normalized = normalize_name(raw);
    let structure = match normalized.as_str() {
        "desert_pyramid" | "desert_temple" => StructureType::Desert_Pyramid,
        "jungle_temple" | "jungle_pyramid" => StructureType::Jungle_Temple,
        "swamp_hut" | "witch_hut" => StructureType::Swamp_Hut,
        "igloo" => StructureType::Igloo,
        "village" => StructureType::Village,
        "ocean_ruin" => StructureType::Ocean_Ruin,
        "shipwreck" => StructureType::Shipwreck,
        "monument" | "ocean_monument" => StructureType::Monument,
        "mansion" | "woodland_mansion" => StructureType::Mansion,
        "outpost" | "pillager_outpost" => StructureType::Outpost,
        "ruined_portal" => StructureType::Ruined_Portal,
        "ruined_portal_nether" | "ruined_portal_n" => StructureType::Ruined_Portal_N,
        "ancient_city" => StructureType::Ancient_City,
        "treasure" | "buried_treasure" => StructureType::Treasure,
        "mineshaft" => StructureType::Mineshaft,
        "desert_well" => StructureType::Desert_Well,
        "geode" | "amethyst_geode" => StructureType::Geode,
        "fortress" | "nether_fortress" => StructureType::Fortress,
        "bastion" | "bastion_remnant" => StructureType::Bastion,
        "end_city" => StructureType::End_City,
        "end_gateway" => StructureType::End_Gateway,
        "end_island" => StructureType::End_Island,
        "trail_ruins" => StructureType::Trail_Ruins,
        "trial_chambers" => StructureType::Trial_Chambers,
        _ => bail!("不支持的结构 '{raw}'"),
    };

    Ok(structure)
}
