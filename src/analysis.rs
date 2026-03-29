use cubiomes::{
    enums::{BiomeID, MCVersion, StructureType},
    generator::{BlockPosition, Generator},
};

use crate::{
    scoring::{
        AnimalType, FarmingAnalysis, FoodSource, MobTowerLocation, NearbyResources, SpawnAnalysis,
        SurfaceType,
    },
    types::{LocatedStructure, is_ocean_biome, is_water_biome, surface_y},
};

pub fn analyze_spawn(
    generator: &Generator,
    version: MCVersion,
    spawn: BlockPosition,
    biomes: &[(BiomeID, impl Clone + Copy)],
) -> SpawnAnalysis {
    let mut issues = Vec::new();
    let mut advantages = Vec::new();

    let y = surface_y(version);

    let spawn_biome = generator.get_biome_at(spawn.x, y, spawn.z).ok();

    let is_spawn_in_water = spawn_biome.map(|b| is_water_biome(b)).unwrap_or(false);

    // Sample at step=4 instead of every block: ~32×32=1024 vs 129×129=16641 calls
    let step = 4;
    let radius = 64;
    let mut water_count = 0u32;
    let mut total_count = 0u32;

    let mut dx = -radius;
    while dx <= radius {
        let mut dz = -radius;
        while dz <= radius {
            let x = spawn.x + dx;
            let z = spawn.z + dz;
            if let Ok(biome) = generator.get_biome_at(x, y, z) {
                total_count += 1;
                if is_water_biome(biome) {
                    water_count += 1;
                }
            }
            dz += step;
        }
        dx += step;
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

/// Spiral outward from spawn to find the nearest land block.
/// Stops as soon as a non-ocean block is found instead of scanning whole rings.
fn find_nearest_land(
    generator: &Generator,
    spawn: BlockPosition,
    version: MCVersion,
) -> Option<f32> {
    let y = surface_y(version);
    let step = 8;
    let max_radius = 256;

    // Check spawn itself first
    if let Ok(biome) = generator.get_biome_at(spawn.x, y, spawn.z) {
        if !is_ocean_biome(biome) {
            return Some(0.0);
        }
    }

    // Expand in concentric rings
    let mut r = step;
    while r <= max_radius {
        let mut best_dist_sq: Option<i64> = None;

        // Scan the perimeter of this ring
        let mut d = -r;
        while d <= r {
            // Top edge (z = -r)
            check_land(generator, spawn, d, -r, y, &mut best_dist_sq);
            // Bottom edge (z = +r)
            check_land(generator, spawn, d, r, y, &mut best_dist_sq);
            // Left edge (x = -r), skip corners already checked
            if d != -r && d != r {
                check_land(generator, spawn, -r, d, y, &mut best_dist_sq);
            }
            // Right edge (x = +r)
            if d != -r && d != r {
                check_land(generator, spawn, r, d, y, &mut best_dist_sq);
            }
            d += step;
        }

        if let Some(dist_sq) = best_dist_sq {
            return Some((dist_sq as f32).sqrt());
        }
        r += step;
    }

    None
}

fn check_land(
    generator: &Generator,
    spawn: BlockPosition,
    dx: i32,
    dz: i32,
    y: i32,
    best_dist_sq: &mut Option<i64>,
) {
    let x = spawn.x + dx;
    let z = spawn.z + dz;
    if let Ok(biome) = generator.get_biome_at(x, y, z) {
        if !is_ocean_biome(biome) {
            let d = (dx as i64) * (dx as i64) + (dz as i64) * (dz as i64);
            match best_dist_sq {
                Some(prev) if *prev <= d => {}
                _ => *best_dist_sq = Some(d),
            }
        }
    }
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

    for dx in (-search_radius..=search_radius).step_by(64) {
        for dz in (-search_radius..=search_radius).step_by(64) {
            let x = spawn.x + dx;
            let z = spawn.z + dz;

            if let Ok(biome) = generator.get_biome_at(x, y, z) {
                let ocean = is_ocean_biome(biome);

                let surface_type = if ocean {
                    SurfaceType::Ocean
                } else {
                    match biome {
                        BiomeID::desert => SurfaceType::Desert,
                        BiomeID::swamp => SurfaceType::Swamp,
                        _ => SurfaceType::Land,
                    }
                };

                let distance = ((dx * dx + dz * dz) as f32).sqrt();

                if distance < 200.0 || (ocean && distance < 500.0) {
                    locations.push(MobTowerLocation {
                        position: BlockPosition::new(x, z),
                        height_estimate: if ocean { 62 } else { 128 },
                        surface_type,
                        is_ocean: ocean,
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
