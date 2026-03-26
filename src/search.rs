use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use cubiomes::{
    enums::{BiomeID, Dimension, MCVersion, StructureType},
    generator::{BlockPosition, Generator, GeneratorFlags},
    noise::{BiomeNoise, SurfaceNoiseRelease},
    structures::StructureRegion,
};
use cubiomes_sys::getSpawn;

use crate::{
    config::{search_worker_count, total_seeds, SearchConfig},
    log_diag,
    types::{
        block_distance, LocatedBiome, LocatedStructure, MatchSummary, SearchEvent,
        TerrainStats, WorkerMessage,
    },
};

pub fn run_search(
    config: SearchConfig,
    cancel: Arc<AtomicBool>,
    tx: std::sync::mpsc::Sender<WorkerMessage>,
) -> Result<()> {
    log_diag(&format!(
        "run_search_begin range={}..={} version={}",
        config.seed_start, config.seed_end, config.version_label
    ));
    let total = total_seeds(&config);
    let needs_biome_scan =
        !config.required_biomes.is_empty() || !config.forbidden_biomes.is_empty();
    let needs_terrain_scan = config.min_avg_height.is_some()
        || config.max_avg_height.is_some()
        || config.max_relief.is_some();
    let worker_count = search_worker_count(&config);
    let searched = Arc::new(AtomicUsize::new(0));
    let found = Arc::new(AtomicUsize::new(0));
    let last_seed = Arc::new(AtomicI64::new(config.seed_start));
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    let mut handles = Vec::with_capacity(worker_count);

    for worker_index in 0..worker_count {
        let config = config.clone();
        let cancel = Arc::clone(&cancel);
        let searched = Arc::clone(&searched);
        let found = Arc::clone(&found);
        let last_seed = Arc::clone(&last_seed);
        let event_tx = event_tx.clone();

        handles.push(thread::spawn(move || {
            let worker_result: Result<()> = (|| {
                let stride = worker_count as i64;
                let mut seed = config.seed_start + worker_index as i64;

                while seed <= config.seed_end {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }

                    last_seed.fetch_max(seed, Ordering::Relaxed);
                    searched.fetch_add(1, Ordering::Relaxed);

                    if let Some(summary) =
                        evaluate_seed(seed, &config, needs_biome_scan, needs_terrain_scan)?
                    {
                        log_diag(&format!("run_search_match seed={seed}"));
                        let slot = found.fetch_add(1, Ordering::Relaxed) + 1;
                        if slot <= config.limit {
                            let _ = event_tx.send(SearchEvent::Match(summary));
                            if slot == config.limit {
                                cancel.store(true, Ordering::Relaxed);
                                break;
                            }
                        } else {
                            cancel.store(true, Ordering::Relaxed);
                            break;
                        }
                    }

                    seed += stride;
                }

                Ok(())
            })();

            if let Err(err) = worker_result {
                cancel.store(true, Ordering::Relaxed);
                let _ = event_tx.send(SearchEvent::Error(err.to_string()));
            }

            let _ = event_tx.send(SearchEvent::WorkerDone);
        }));
    }
    drop(event_tx);

    let mut completed_workers = 0usize;
    let mut last_progress_sent = Instant::now() - Duration::from_millis(120);
    let mut worker_error: Option<String> = None;

    while completed_workers < worker_count {
        match event_rx.recv_timeout(Duration::from_millis(40)) {
            Ok(SearchEvent::Match(summary)) => {
                let _ = tx.send(WorkerMessage::Match(summary));
            }
            Ok(SearchEvent::Error(err)) => {
                worker_error = Some(err);
            }
            Ok(SearchEvent::WorkerDone) => {
                completed_workers += 1;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }

        if last_progress_sent.elapsed() >= Duration::from_millis(75) {
            let searched_now = searched.load(Ordering::Relaxed);
            let found_now = found.load(Ordering::Relaxed).min(config.limit);
            let current_seed = last_seed.load(Ordering::Relaxed);
            let _ = tx.send(WorkerMessage::Progress {
                searched: searched_now,
                total,
                found: found_now,
                current_seed,
            });
            last_progress_sent = Instant::now();
        }

        if worker_error.is_some() {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    for handle in handles {
        let _ = handle.join();
    }

    if let Some(err) = worker_error {
        return Err(anyhow::anyhow!(err));
    }

    let searched_now = searched.load(Ordering::Relaxed);
    let found_now = found.load(Ordering::Relaxed).min(config.limit);
    let current_seed = last_seed.load(Ordering::Relaxed);
    let _ = tx.send(WorkerMessage::Progress {
        searched: searched_now,
        total,
        found: found_now,
        current_seed,
    });
    let _ = tx.send(WorkerMessage::Finished {
        searched: searched_now,
        found: found_now,
        stopped: cancel.load(Ordering::Relaxed) && found_now < config.limit,
    });
    log_diag(&format!(
        "run_search_finished searched={} found={} workers={}",
        searched_now, found_now, worker_count
    ));

    Ok(())
}

fn evaluate_seed(
    seed: i64,
    config: &SearchConfig,
    needs_biome_scan: bool,
    needs_terrain_scan: bool,
) -> Result<Option<MatchSummary>> {
    let mut generator = Generator::new(
        config.version,
        seed,
        Dimension::DIM_OVERWORLD,
        GeneratorFlags::empty(),
    );
    let spawn = get_spawn(&generator);

    let terrain = if needs_terrain_scan {
        let stats = sample_terrain(
            &generator,
            config.version,
            seed,
            spawn,
            config.terrain_radius,
        )?;
        if let Some(min_avg_height) = config.min_avg_height
            && stats.avg_height < min_avg_height
        {
            return Ok(None);
        }
        if let Some(max_avg_height) = config.max_avg_height
            && stats.avg_height > max_avg_height
        {
            return Ok(None);
        }
        if let Some(max_relief) = config.max_relief
            && stats.relief > max_relief
        {
            return Ok(None);
        }
        Some(stats)
    } else {
        None
    };

    let biome_hits = if needs_biome_scan {
        let biome_map = scan_biomes(
            &generator,
            config.version,
            spawn,
            config.biome_radius,
            config.biome_step,
            &config.required_biomes,
            &config.forbidden_biomes,
        )?;

        if config
            .required_biomes
            .iter()
            .any(|biome| !biome_map.contains_key(biome))
        {
            return Ok(None);
        }
        if config
            .forbidden_biomes
            .iter()
            .any(|biome| biome_map.contains_key(biome))
        {
            return Ok(None);
        }

        config
            .required_biomes
            .iter()
            .filter_map(|biome| biome_map.get(biome).copied().map(|hit| (*biome, hit)))
            .collect()
    } else {
        Vec::new()
    };

    let mut structure_hits = Vec::new();
    for structure in &config.required_structures {
        let Some(hit) = find_nearest_structure(
            &mut generator,
            config.version,
            spawn,
            config.structure_radius,
            *structure,
        )?
        else {
            return Ok(None);
        };
        structure_hits.push((*structure, hit));
    }

    Ok(Some(MatchSummary {
        seed,
        version: config.version,
        version_label: config.version_label.clone(),
        version_warning: config.version_warning.clone(),
        spawn,
        terrain,
        biomes: biome_hits,
        structures: structure_hits,
        is_favorite: false,
    }))
}

pub fn get_spawn(generator: &Generator) -> BlockPosition {
    let pos = unsafe { getSpawn(generator.as_ptr()) };
    BlockPosition::new(pos.x, pos.z)
}

fn sample_terrain(
    generator: &Generator,
    version: MCVersion,
    seed: i64,
    spawn: BlockPosition,
    radius: i32,
) -> Result<TerrainStats> {
    let start_x = (spawn.x - radius).div_euclid(4);
    let start_z = (spawn.z - radius).div_euclid(4);
    let size = ((radius * 2).div_euclid(4) + 1) as u32;
    let surface_noise = SurfaceNoiseRelease::new(Dimension::DIM_OVERWORLD, seed);
    let noise: BiomeNoise = surface_noise.into();
    let heights = generator
        .approx_surface_noise(start_x, start_z, size, size, &noise)
        .with_context(|| format!("failed to sample terrain for seed {seed}"))?;

    let mut min_height = f32::INFINITY;
    let mut max_height = f32::NEG_INFINITY;
    let mut sum = 0.0f64;

    for height in heights {
        min_height = min_height.min(height);
        max_height = max_height.max(height);
        sum += height as f64;
    }

    let avg_height = (sum / (size * size) as f64) as f32;
    let relief = max_height - min_height;

    let _ = version;

    Ok(TerrainStats {
        min_height,
        max_height,
        avg_height,
        relief,
    })
}

fn scan_biomes(
    generator: &Generator,
    version: MCVersion,
    spawn: BlockPosition,
    radius: i32,
    step: i32,
    required_biomes: &[BiomeID],
    forbidden_biomes: &[BiomeID],
) -> Result<HashMap<BiomeID, LocatedBiome>> {
    let tracked: Vec<BiomeID> = required_biomes
        .iter()
        .chain(forbidden_biomes.iter())
        .copied()
        .collect();
    let mut found = HashMap::new();
    let y = surface_y(version);

    for x in (spawn.x - radius..=spawn.x + radius).step_by(step as usize) {
        for z in (spawn.z - radius..=spawn.z + radius).step_by(step as usize) {
            let biome = generator
                .get_biome_at(x, y, z)
                .with_context(|| format!("failed to get biome at ({x}, {z})"))?;

            if !tracked.contains(&biome) {
                continue;
            }

            let position = BlockPosition::new(x, z);
            let distance = block_distance(spawn, position);
            let entry = found
                .entry(biome)
                .or_insert(LocatedBiome { position, distance });
            if distance < entry.distance {
                *entry = LocatedBiome { position, distance };
            }
        }
    }

    Ok(found)
}

fn find_nearest_structure(
    generator: &mut Generator,
    version: MCVersion,
    spawn: BlockPosition,
    radius: i32,
    structure: StructureType,
) -> Result<Option<LocatedStructure>> {
    let center_region = StructureRegion::from_block_position(spawn, version, structure)
        .with_context(|| format!("structure {structure} is not supported on {version}"))?;
    let region_span = center_region.region_size_blocks().max(1);
    let search_range = radius.div_euclid(region_span) + 2;
    let mut best: Option<LocatedStructure> = None;

    for region_x in center_region.x - search_range..=center_region.x + search_range {
        for region_z in center_region.z - search_range..=center_region.z + search_range {
            let region = StructureRegion::new(region_x, region_z, version, structure)
                .with_context(|| format!("failed to create search region for {structure}"))?;

            let Some(position) = generator.try_generate_structure_in_region(region) else {
                continue;
            };

            let distance = block_distance(spawn, position);
            if distance > radius as f64 {
                continue;
            }

            match best {
                Some(current) if current.distance <= distance => {}
                _ => {
                    best = Some(LocatedStructure { position, distance });
                }
            }
        }
    }

    Ok(best)
}

fn surface_y(version: MCVersion) -> i32 {
    if (version as i32) >= (MCVersion::MC_1_18_2 as i32) {
        320
    } else {
        255
    }
}

pub fn with_cubiomes_lock<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    static CUBIOMES_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let lock = CUBIOMES_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock
        .lock()
        .map_err(|_| anyhow::anyhow!("cubiomes global lock poisoned"))?;
    f()
}
