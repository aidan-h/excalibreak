use excali_io::SerializeKey;
use parry3d::bounding_volume::Aabb;
use std::collections::HashMap;

use excali_io::FromKeyError;
use nalgebra::{SMatrix, Vector2, Vector3};

use serde::{Deserialize, Serialize};

pub const CHUNK_SIZE: usize = 64;
pub const MAP_FILE_PATH: &str = "assets/map.toml";

#[derive(Serialize, Deserialize, Clone)]
pub struct Zone {
    pub level_name: String,
}

pub struct Grid {
    pub height_map: SMatrix<u16, CHUNK_SIZE, CHUNK_SIZE>,
    pub zones: HashMap<Vector2<u16>, Zone>,
}

impl TryInto<Grid> for SerializableGrid {
    type Error = FromKeyError<u16>;
    fn try_into(self) -> Result<Grid, Self::Error> {
        Ok(Grid {
            height_map: self.height_map,
            zones: Vector2::<u16>::deserialize_hash_map(&self.zones)?,
        })
    }
}

impl From<Grid> for SerializableGrid {
    fn from(val: Grid) -> Self {
        SerializableGrid {
            height_map: val.height_map,
            zones: Vector2::<u16>::serialize_hash_map(&val.zones),
        }
    }
}

impl From<&Grid> for SerializableGrid {
    fn from(val: &Grid) -> Self {
        SerializableGrid {
            height_map: val.height_map,
            zones: Vector2::<u16>::serialize_hash_map(&val.zones),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct SerializableGrid {
    pub height_map: SMatrix<u16, CHUNK_SIZE, CHUNK_SIZE>,
    pub zones: HashMap<String, Zone>,
}

impl Default for Grid {
    fn default() -> Self {
        let mut height_map = SMatrix::<u16, CHUNK_SIZE, CHUNK_SIZE>::zeros();
        height_map.fill(1);

        Self {
            height_map,
            zones: HashMap::new(),
        }
    }
}

pub const ZONE_SIZE: Vector3<f32> = Vector3::new(1.0, 1.0, 1.0);
pub const HALF_ZONE_SIZE: Vector3<f32> = Vector3::new(0.5, 0.5, 0.5);

impl Grid {
    /// returns the center of a zone object
    pub fn zone_world_position(&self, coordinate: &Vector2<u16>) -> Vector3<f32> {
        let height = match self
            .height_map
            .row(coordinate.y as usize)
            .get(coordinate.x as usize)
        {
            Some(height) => *height,
            None => 0,
        };
        Vector3::new(
            coordinate.x as f32,
            height as f32 + 0.5,
            coordinate.y as f32,
        )
    }

    pub fn zone_aabb(&self, coordinate: &Vector2<u16>) -> Aabb {
        let position = self.zone_world_position(coordinate);
        Aabb::new(
            (position - HALF_ZONE_SIZE).into(),
            (position + HALF_ZONE_SIZE).into(),
        )
    }
}
