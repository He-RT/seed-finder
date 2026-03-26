use anyhow::Result;
use cubiomes::{
    colors::BiomeColorMap,
    enums::Dimension,
    generator::{Generator, GeneratorFlags},
};
use eframe::egui::Color32;

use crate::{
    log_diag,
    search::with_cubiomes_lock,
    types::{MatchSummary, PreviewRequest},
};

pub fn generate_preview(request: &PreviewRequest) -> Result<eframe::egui::ColorImage> {
    log_diag(&format!("generate_preview seed={}", request.summary.seed));
    with_cubiomes_lock(|| generate_preview_locked(request))
}

fn generate_preview_locked(request: &PreviewRequest) -> Result<eframe::egui::ColorImage> {
    let mut generator = Generator::new(
        request.summary.version,
        request.summary.seed,
        Dimension::DIM_OVERWORLD,
        GeneratorFlags::empty(),
    );
    let colors = BiomeColorMap::new();
    let size = request.image_size;
    let mut pixels = vec![Color32::from_rgb(4, 8, 10); size * size];
    let diameter = request.radius * 2;
    let surface_y = surface_y(request.summary.version);

    for py in 0..size {
        let world_z = request.summary.spawn.z - request.radius
            + ((py as f32 / (size - 1) as f32) * diameter as f32).round() as i32;
        for px in 0..size {
            let world_x = request.summary.spawn.x - request.radius
                + ((px as f32 / (size - 1) as f32) * diameter as f32).round() as i32;
            let biome = generator.get_biome_at(world_x, surface_y, world_z)?;
            let [r, g, b] = colors[biome];
            pixels[py * size + px] = Color32::from_rgb(r, g, b);
        }
    }

    paint_cross(
        &mut pixels,
        size,
        world_to_pixel(
            request.summary.spawn.x,
            request.summary.spawn.x,
            request.radius,
            size,
        ),
        world_to_pixel(
            request.summary.spawn.z,
            request.summary.spawn.z,
            request.radius,
            size,
        ),
        Color32::BLACK,
        6,
    );
    paint_cross(
        &mut pixels,
        size,
        world_to_pixel(
            request.summary.spawn.x,
            request.summary.spawn.x,
            request.radius,
            size,
        ),
        world_to_pixel(
            request.summary.spawn.z,
            request.summary.spawn.z,
            request.radius,
            size,
        ),
        Color32::WHITE,
        4,
    );

    for (_, hit) in &request.summary.biomes {
        paint_dot(
            &mut pixels,
            size,
            world_to_pixel(
                hit.position.x,
                request.summary.spawn.x,
                request.radius,
                size,
            ),
            world_to_pixel(
                hit.position.z,
                request.summary.spawn.z,
                request.radius,
                size,
            ),
            Color32::from_rgb(109, 224, 210),
            3,
        );
    }

    for (_, hit) in &request.summary.structures {
        paint_cross(
            &mut pixels,
            size,
            world_to_pixel(
                hit.position.x,
                request.summary.spawn.x,
                request.radius,
                size,
            ),
            world_to_pixel(
                hit.position.z,
                request.summary.spawn.z,
                request.radius,
                size,
            ),
            Color32::from_rgb(255, 192, 92),
            4,
        );
    }

    let _ = &mut generator;
    Ok(eframe::egui::ColorImage::new([size, size], pixels))
}

fn surface_y(version: cubiomes::enums::MCVersion) -> i32 {
    if (version as i32) >= (cubiomes::enums::MCVersion::MC_1_18_2 as i32) {
        320
    } else {
        255
    }
}

fn world_to_pixel(world: i32, center: i32, radius: i32, size: usize) -> i32 {
    let min = center - radius;
    let span = (radius * 2).max(1) as f32;
    let normalized = ((world - min) as f32 / span).clamp(0.0, 1.0);
    (normalized * (size.saturating_sub(1)) as f32).round() as i32
}

fn paint_cross(pixels: &mut [Color32], size: usize, x: i32, y: i32, color: Color32, radius: i32) {
    for dx in -radius..=radius {
        set_pixel(pixels, size, x + dx, y, color);
    }
    for dy in -radius..=radius {
        set_pixel(pixels, size, x, y + dy, color);
    }
}

fn paint_dot(pixels: &mut [Color32], size: usize, x: i32, y: i32, color: Color32, radius: i32) {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= radius * radius {
                set_pixel(pixels, size, x + dx, y + dy, color);
            }
        }
    }
}

fn set_pixel(pixels: &mut [Color32], size: usize, x: i32, y: i32, color: Color32) {
    if x < 0 || y < 0 {
        return;
    }
    let x = x as usize;
    let y = y as usize;
    if x >= size || y >= size {
        return;
    }
    pixels[y * size + x] = color;
}

pub fn generate_preview_for_summary(
    summary: &MatchSummary,
    radius: i32,
    image_size: usize,
) -> Result<eframe::egui::ColorImage> {
    let request = PreviewRequest {
        token: 0,
        summary: summary.clone(),
        radius,
        image_size,
    };
    generate_preview(&request)
}
