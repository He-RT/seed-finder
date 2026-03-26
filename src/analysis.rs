use cubiomes::{
    enums::{BiomeID, MCVersion, StructureType},
    generator::{BlockPosition, Generator},
};

use crate::{
    scoring::{
        AnimalType, FarmingAnalysis, FoodSource, MobTowerLocation, NearbyResources, SpawnAnalysis,
        SurfaceType,
    },
    types::LocatedStructure,
};

pub fn analyze_spawn(
    generator: &Generator,
    version: MCVersion,
    spawn: BlockPosition,
    biomes: &[(BiomeID, impl Clone + Copy)],
) -> SpawnAnalysis {
    let mut issues = Vec::new();
    let mut advantages = Vec::new();

    let water_biomes: std::collections::HashSet<BiomeID> = [
        BiomeID::ocean,
        BiomeID::deep_ocean,
        BiomeID::warm_ocean,
        BiomeID::cold_ocean,
        BiomeID::frozen_ocean,
        BiomeID::lukewarm_ocean,
        BiomeID::deep_warm_ocean,
        BiomeID::deep_cold_ocean,
        BiomeID::deep_frozen_ocean,
        BiomeID::river,
    ]
    .iter()
    .copied()
    .collect();

    let spawn_biome = generator
        .get_biome_at(spawn.x, surface_y(version), spawn.z)
        .ok();

    let is_spawn_in_water = spawn_biome
        .map(|b| water_biomes.contains(&b))
        .unwrap_or(false);

    let mut water_count = 0;
    let mut total_count = 0;
    let radius = 64;
    let y = surface_y(version);

    for dx in -radius..=radius {
        for dz in -radius..=radius {
            let x = spawn.x + dx;
            let z = spawn.z + dz;
            if let Ok(biome) = generator.get_biome_at(x, y, z) {
                total_count += 1;
                if water_biomes.contains(&biome) {
                    water_count += 1;
                }
            }
        }
    }

    let water_percentage = if total_count > 0 {
        water_count as f32 / total_count as f32
    } else {
        0.0
    };

    let land_distance = find_nearest_land(generator, spawn, version);

    let is_safe = !is_spawn_in_water && water_percentage < 0.6;
    let has_land_nearby = land_distance.map(|d| d < 100.0).unwrap_or(true);

    if is_spawn_in_water {
        issues.push("出生点位于水中".into());
    }
    if water_percentage > 0.8 {
        issues.push("周围大部分是水域".into());
    }
    if !has_land_nearby {
        issues.push("附近没有陆地".into());
    }
    if water_percentage < 0.2 {
        advantages.push("陆地资源丰富".into());
    }
    if let Some(biome) = spawn_biome {
        if matches!(
            biome,
            BiomeID::plains
                | BiomeID::forest
                | BiomeID::birch_forest
                | BiomeID::meadow
                | BiomeID::sunflower_plains
        ) {
            advantages.push("出生在友好群系".into());
        }
    }

    let nearby_resources = analyze_nearby_resources(generator, version, spawn, biomes);

    SpawnAnalysis {
        is_safe,
        has_land_nearby,
        water_percentage,
        land_distance,
        issues,
        advantages,
        nearby_resources,
    }
}

fn find_nearest_land(
    generator: &Generator,
    spawn: BlockPosition,
    version: MCVersion,
) -> Option<f32> {
    let water_biomes: std::collections::HashSet<BiomeID> = [
        BiomeID::ocean,
        BiomeID::deep_ocean,
        BiomeID::warm_ocean,
        BiomeID::cold_ocean,
        BiomeID::frozen_ocean,
        BiomeID::lukewarm_ocean,
        BiomeID::deep_warm_ocean,
        BiomeID::deep_cold_ocean,
        BiomeID::deep_frozen_ocean,
    ]
    .iter()
    .copied()
    .collect();

    let y = surface_y(version);

    for radius in [16, 32, 64, 128, 256] {
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let x = spawn.x + dx;
                let z = spawn.z + dz;
                if let Ok(biome) = generator.get_biome_at(x, y, z) {
                    if !water_biomes.contains(&biome) {
                        let distance = ((dx * dx + dz * dz) as f32).sqrt();
                        return Some(distance);
                    }
                }
            }
        }
    }

    None
}

fn analyze_nearby_resources(
    generator: &Generator,
    version: MCVersion,
    spawn: BlockPosition,
    _biomes: &[(BiomeID, impl Clone + Copy)],
) -> NearbyResources {
    let mut resources = NearbyResources::default();
    let y = surface_y(version);
    let radius = 128;

    let mut biomes_nearby = std::collections::HashSet::new();

    for dx in (-radius..=radius).step_by(16) {
        for dz in (-radius..=radius).step_by(16) {
            let x = spawn.x + dx;
            let z = spawn.z + dz;
            if let Ok(biome) = generator.get_biome_at(x, y, z) {
                biomes_nearby.insert(biome);
            }
        }
    }

    let forest_biomes: std::collections::HashSet<BiomeID> = [
        BiomeID::forest,
        BiomeID::birch_forest,
        BiomeID::dark_forest,
        BiomeID::taiga,
        BiomeID::jungle,
        BiomeID::cherry_grove,
        BiomeID::pale_garden,
    ]
    .iter()
    .copied()
    .collect();

    if biomes_nearby.iter().any(|b| forest_biomes.contains(b)) {
        resources.trees = true;
    }

    if biomes_nearby.contains(&BiomeID::river)
        || biomes_nearby.contains(&BiomeID::ocean)
        || biomes_nearby.contains(&BiomeID::beach)
    {
        resources.water_source = true;
    }

    for biome in &biomes_nearby {
        match biome {
            BiomeID::plains | BiomeID::sunflower_plains => {
                resources.animals.push(AnimalType::Horse);
                resources.animals.push(AnimalType::Sheep);
                resources.animals.push(AnimalType::Pig);
                resources.animals.push(AnimalType::Cow);
            }
            BiomeID::forest | BiomeID::birch_forest => {
                resources.animals.push(AnimalType::Sheep);
                resources.animals.push(AnimalType::Pig);
                resources.animals.push(AnimalType::Chicken);
                resources.animals.push(AnimalType::Cow);
            }
            BiomeID::taiga => {
                resources.animals.push(AnimalType::Wolf);
                resources.animals.push(AnimalType::Fox);
                resources.animals.push(AnimalType::Rabbit);
            }
            BiomeID::jungle => {
                resources.animals.push(AnimalType::Panda);
                resources.animals.push(AnimalType::Parrot);
            }
            BiomeID::savanna => {
                resources.animals.push(AnimalType::Horse);
                resources.animals.push(AnimalType::Llama);
            }
            BiomeID::swamp => {
                resources.animals.push(AnimalType::Frog);
            }
            _ => {}
        }

        match biome {
            BiomeID::taiga => resources.food_sources.push(FoodSource::SweetBerries),
            BiomeID::plains | BiomeID::savanna => {
                resources.food_sources.push(FoodSource::Wheat);
            }
            BiomeID::desert => {
                resources.food_sources.push(FoodSource::Carrots);
                resources.food_sources.push(FoodSource::Potatoes);
            }
            BiomeID::jungle => {
                resources.food_sources.push(FoodSource::Melon);
            }
            BiomeID::swamp => {
                resources.food_sources.push(FoodSource::Berries);
            }
            BiomeID::river | BiomeID::ocean => {
                resources.food_sources.push(FoodSource::Fish);
            }
            _ => {}
        }
    }

    resources.animals.sort();
    resources.animals.dedup();
    resources.food_sources.sort();
    resources.food_sources.dedup();

    resources
}

pub fn analyze_farming_potential(
    generator: &mut Generator,
    version: MCVersion,
    spawn: BlockPosition,
    structures: &[(StructureType, LocatedStructure)],
    biomes: &[(BiomeID, impl Clone + Copy)],
) -> FarmingAnalysis {
    let mut iron_golem_villages = Vec::new();
    let mut suitable_mob_tower_locations = Vec::new();
    let mut special_biomes_for_farms = Vec::new();

    for (structure, hit) in structures {
        if *structure == StructureType::Village {
            iron_golem_villages.push(*hit);
        }
    }

    let mob_tower_spots = find_mob_tower_locations(generator, version, spawn);
    suitable_mob_tower_locations.extend(mob_tower_spots);

    for (biome, _) in biomes {
        match biome {
            BiomeID::mushroom_fields => {
                special_biomes_for_farms.push((*biome, "蘑菇岛刷怪塔".into()));
            }
            BiomeID::deep_dark => {
                special_biomes_for_farms.push((*biome, "深暗之域守卫者农场".into()));
            }
            BiomeID::swamp => {
                special_biomes_for_farms.push((*biome, "沼泽史莱姆农场".into()));
            }
            BiomeID::dripstone_caves => {
                special_biomes_for_farms.push((*biome, "溶洞石刷农场".into()));
            }
            BiomeID::warm_ocean => {
                special_biomes_for_farms.push((*biome, "温水珊瑚农场".into()));
            }
            _ => {}
        }
    }

    FarmingAnalysis {
        iron_golem_villages,
        suitable_mob_tower_locations,
        special_biomes_for_farms,
    }
}

fn find_mob_tower_locations(
    generator: &mut Generator,
    version: MCVersion,
    spawn: BlockPosition,
) -> Vec<MobTowerLocation> {
    let mut locations = Vec::new();
    let y = surface_y(version);
    let search_radius = 256;

    let ocean_biomes: std::collections::HashSet<BiomeID> = [
        BiomeID::ocean,
        BiomeID::deep_ocean,
        BiomeID::warm_ocean,
        BiomeID::cold_ocean,
        BiomeID::frozen_ocean,
        BiomeID::lukewarm_ocean,
    ]
    .iter()
    .copied()
    .collect();

    for dx in (-search_radius..=search_radius).step_by(64) {
        for dz in (-search_radius..=search_radius).step_by(64) {
            let x = spawn.x + dx;
            let z = spawn.z + dz;

            if let Ok(biome) = generator.get_biome_at(x, y, z) {
                let is_ocean = ocean_biomes.contains(&biome);

                let surface_type = if is_ocean {
                    SurfaceType::Ocean
                } else {
                    match biome {
                        BiomeID::desert => SurfaceType::Desert,
                        BiomeID::swamp => SurfaceType::Swamp,
                        _ => SurfaceType::Land,
                    }
                };

                let distance = ((dx * dx + dz * dz) as f32).sqrt();

                if distance < 200.0 || (is_ocean && distance < 500.0) {
                    locations.push(MobTowerLocation {
                        position: BlockPosition::new(x, z),
                        height_estimate: if is_ocean { 62 } else { 128 },
                        surface_type,
                        is_ocean,
                    });
                }
            }
        }
    }

    locations
}

pub fn calculate_nether_coords(overworld_x: i32, overworld_z: i32) -> (i32, i32) {
    (overworld_x / 8, overworld_z / 8)
}

pub fn format_nether_info(overworld_pos: BlockPosition) -> String {
    let (nx, nz) = calculate_nether_coords(overworld_pos.x, overworld_pos.z);
    format!("下界坐标: ({}, {})", nx, nz)
}

pub fn format_teleport_command(pos: BlockPosition, dimension: &str) -> String {
    match dimension {
        "overworld" => format!("/tp @p {} 64 {}", pos.x, pos.z),
        "nether" => {
            let (nx, nz) = calculate_nether_coords(pos.x, pos.z);
            format!("/tp @p {} 64 {} in minecraft:the_nether", nx, nz)
        }
        _ => format!("/tp @p {} 64 {}", pos.x, pos.z),
    }
}

pub fn format_coordinate_copy(pos: BlockPosition) -> String {
    format!("X: {}, Y: ~, Z: {}", pos.x, pos.z)
}

fn surface_y(version: MCVersion) -> i32 {
    if (version as i32) >= (MCVersion::MC_1_18_2 as i32) {
        320
    } else {
        255
    }
}
