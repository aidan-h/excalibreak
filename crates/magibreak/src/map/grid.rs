use nalgebra::SMatrix;
use perlin_noise::PerlinNoise;

use serde::{Deserialize, Serialize};

pub const CHUNK_SIZE: usize = 64;
pub const MAP_FILE_PATH: &str = "assets/map.toml";

#[derive(Serialize, Deserialize)]
pub struct Grid {
    pub height_map: SMatrix<u16, CHUNK_SIZE, CHUNK_SIZE>,
}

impl Default for Grid {
    fn default() -> Self {
        let mut height_map = SMatrix::<u16, CHUNK_SIZE, CHUNK_SIZE>::zeros();
        height_map.fill(1);

        Self { height_map }
    }
}

impl Grid {
    pub fn from_noise() -> Self {
        let mut height_map = SMatrix::<u16, CHUNK_SIZE, CHUNK_SIZE>::zeros();

        let perlin = PerlinNoise::new();
        for (y, mut row) in height_map.row_iter_mut().enumerate() {
            for (x, cell) in row.iter_mut().enumerate() {
                let noise: f64 = perlin.get2d([x as f64 / 10.0, y as f64 / 10.0]) * 7.0;
                *cell = noise.floor() as u16;
            }
        }
        println!(
            "{}",
            excali_io::toml::to_string(&Self { height_map }).unwrap()
        );

        Self { height_map }
    }
}
