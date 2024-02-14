use excali_io::SerializeKey;
use parry3d::bounding_volume::Aabb;
use std::collections::HashMap;

use excali_io::FromKeyError;
use nalgebra::{SMatrix, Vector2, Vector3};

use serde::{Deserialize, Serialize};

pub const CHUNK_SIZE: usize = 64;
pub const MAP_FILE_PATH: &str = "assets/map.toml";
pub type MapCoordinate = Vector2<u16>;

#[derive(Clone, Debug, PartialEq)]
pub enum ZoneState {
    Unlocked,
    Locked,
    Solved,
}

impl ZoneState {
    pub fn selectable(&self) -> bool {
        *self != Self::Locked
    }

    pub fn unlock(&mut self) {
        *self = match *self {
            Self::Solved => Self::Solved,
            _ => Self::Unlocked,
        }
    }
}

/// Zone as loaded in game
#[derive(Clone, Debug)]
pub struct ActiveZone {
    pub zone: Zone,
    pub state: ZoneState,
}

impl Default for ActiveZone {
    fn default() -> Self {
        Self {
            zone: Default::default(),
            state: ZoneState::Locked,
        }
    }
}

/// Zone as stored in map.toml
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Zone {
    pub level_name: String,
    pub next_zones: Vec<MapCoordinate>,
}

impl Default for Zone {
    fn default() -> Self {
        Self {
            level_name: "alpha".to_string(),
            next_zones: Vec::new(),
        }
    }
}

impl From<ActiveZone> for Zone {
    fn from(value: ActiveZone) -> Self {
        value.zone
    }
}

/// Grid as loaded in game
#[derive(Debug)]
pub struct Grid {
    pub height_map: SMatrix<u16, CHUNK_SIZE, CHUNK_SIZE>,
    pub starting_zone: MapCoordinate,
    zones: HashMap<MapCoordinate, ActiveZone>,
}

impl Grid {
    pub fn zones(&self) -> &HashMap<MapCoordinate, ActiveZone> {
        &self.zones
    }

    pub fn delete_zone(&mut self, coordinate: &MapCoordinate) {
        self.zones.remove(coordinate);
    }

    /// NOTE don't use if you can resist it
    pub fn zone_mut(&mut self, coordinate: &MapCoordinate) -> Option<&mut ActiveZone> {
        self.zones.get_mut(coordinate)
    }

    pub fn add_zone(&mut self, coordinate: MapCoordinate) {
        self.zones.insert(coordinate, Default::default());
    }

    pub fn complete_zone(&mut self, coordinate: &MapCoordinate) {
        if let Some(active_zone) = self.zones.get_mut(coordinate) {
            active_zone.state = ZoneState::Solved;

            for next_coordinate in active_zone.zone.next_zones.clone().iter() {
                if let Some(zone) = self.zones.get_mut(next_coordinate) {
                    zone.state.unlock();
                }
            }
        }
    }
}

impl TryInto<Grid> for SerializableGrid {
    type Error = FromKeyError<u16>;
    fn try_into(self) -> Result<Grid, Self::Error> {
        let mut zones = Vector2::<u16>::deserialize_hash_map(&self.zones)?;
        let mut active_zones = HashMap::<MapCoordinate, ActiveZone>::new();
        for (key, zone) in zones.drain() {
            active_zones.insert(
                key,
                ActiveZone {
                    zone,
                    state: if self.starting_zone == key {
                        ZoneState::Unlocked
                    } else {
                        ZoneState::Locked
                    },
                },
            );
        }
        Ok(Grid {
            height_map: self.height_map,
            zones: active_zones,
            starting_zone: self.starting_zone,
        })
    }
}

impl From<&Grid> for SerializableGrid {
    fn from(val: &Grid) -> Self {
        let mut active_zones = Vector2::<u16>::serialize_hash_map(&val.zones);
        let mut zones = HashMap::<String, Zone>::new();
        for (coordinate, zone) in active_zones.drain() {
            zones.insert(coordinate, zone.into());
        }

        SerializableGrid {
            height_map: val.height_map,
            starting_zone: val.starting_zone,
            zones,
        }
    }
}

/// Grid as stored in map.toml
#[derive(Serialize, Deserialize)]
pub struct SerializableGrid {
    pub height_map: SMatrix<u16, CHUNK_SIZE, CHUNK_SIZE>,
    pub zones: HashMap<String, Zone>,
    pub starting_zone: MapCoordinate,
}

impl Default for Grid {
    fn default() -> Self {
        let mut height_map = SMatrix::<u16, CHUNK_SIZE, CHUNK_SIZE>::zeros();
        height_map.fill(1);

        let mut zones = HashMap::<MapCoordinate, Zone>::new();
        let coordinate = Vector2::<u16>::zeros();
        zones.insert(
            coordinate,
            Zone {
                level_name: "alpha".into(),
                next_zones: Vec::new(),
            },
        );

        Self {
            height_map,
            zones: HashMap::new(),
            starting_zone: coordinate,
        }
    }
}

pub const ZONE_SIZE: Vector3<f32> = Vector3::new(1.0, 1.0, 1.0);
pub const HALF_ZONE_SIZE: Vector3<f32> = Vector3::new(0.5, 0.5, 0.5);

impl Grid {
    /// returns the center of a zone object
    pub fn zone_world_position(&self, coordinate: &MapCoordinate) -> Vector3<f32> {
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

    pub fn zone_aabb(&self, coordinate: &MapCoordinate) -> Aabb {
        let position = self.zone_world_position(coordinate);
        Aabb::new(
            (position - HALF_ZONE_SIZE).into(),
            (position + HALF_ZONE_SIZE).into(),
        )
    }
}
