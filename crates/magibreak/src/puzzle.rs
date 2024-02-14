use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f32::consts::PI;
use std::num::ParseIntError;

use excali_sprite::*;
use gcd::Gcd;
use nalgebra::Vector2;

const SIGIL_SIZE: f32 = 50.0;
const SIGIL_SCALE: Vector2<f32> = Vector2::new(SIGIL_SIZE, SIGIL_SIZE);
pub const CURSOR_SIZE: f32 = 70.0;

const SIGIL_DISTANCE: f32 = SIGIL_SIZE * 1.5;

pub trait FromSigilCoordinate {
    fn from_sigil_coordinate(coordinate: SigilCoordinate) -> Self;
}

impl FromSigilCoordinate for Transform {
    fn from_sigil_coordinate(coordinate: SigilCoordinate) -> Self {
        Self {
            position: coordinate.position(),
            rotation: 0.0,
            scale: SIGIL_SCALE,
        }
    }
}

#[derive(Serialize, Debug, PartialEq, Eq, Deserialize, Copy, Clone)]
pub enum Orb {
    Circle,
    Diamond,
    Octogon,
}

impl ToString for Orb {
    fn to_string(&self) -> String {
        match self {
            Self::Circle => "Circle".to_string(),
            Self::Diamond => "Diamond".to_string(),
            Self::Octogon => "Octogon".to_string(),
        }
    }
}

impl Orb {
    fn allow_intersections(&self) -> bool {
        *self == Self::Diamond
    }

    // TODO delete colinear lines, maybe
    fn effect(&self, coordinate: SigilCoordinate, to: SigilCoordinate, lines: &mut [Line]) {
        if *self != Orb::Octogon {
            return;
        }
        for line in lines.iter_mut() {
            if line.end != coordinate {
                continue;
            }
            line.end = to;
        }
    }

    pub fn texture_coordinate(&self, active: bool) -> TextureCoordinate {
        let x = if active { 0.5 } else { 0.0 };
        let y = match self {
            Self::Octogon => 2.0 / 3.0,
            Self::Diamond => 1.0 / 3.0,
            Self::Circle => 0.0,
        };
        TextureCoordinate {
            width: 0.5,
            height: 1.0 / 3.0,
            x,
            y,
        }
    }
}

#[derive(Serialize, Debug, Deserialize, Copy, Clone)]
pub enum Sigil {
    Alpha,
    Sigma,
    Delta,
    Phi,
}

impl ToString for Sigil {
    fn to_string(&self) -> String {
        match self {
            Self::Alpha => "Alpha".to_string(),
            Self::Sigma => "Sigma".to_string(),
            Self::Delta => "Delta".to_string(),
            Self::Phi => "Phi".to_string(),
        }
    }
}

impl Sigil {
    fn active(&self, coordinate: SigilCoordinate, lines: &[Line]) -> bool {
        match self {
            Self::Alpha => {
                // true on connected to a line
                for line in lines.iter() {
                    for touching_coordinate in line.coordinates().iter() {
                        if *touching_coordinate == coordinate {
                            return true;
                        }
                    }
                }
                false
            }
            Self::Phi => {
                // false on connected to a line
                for line in lines.iter() {
                    for touching_coordinate in line.coordinates().iter() {
                        if *touching_coordinate == coordinate {
                            return false;
                        }
                    }
                }
                true
            }
            Self::Delta => {
                // true inside but not on a triangle (direction matters)
                for line_a in lines.iter() {
                    for line_b in lines.iter() {
                        if line_a.extends(line_b) {
                            for line_c in lines.iter() {
                                if line_b.extends(line_c) && line_c.extends(line_a) {
                                    // triangle - directional
                                    let direction =
                                        orientation(line_a.start, line_a.end, line_b.end);
                                    if direction == Orientation::Collinear {
                                        continue;
                                    }

                                    if orientation(line_a.start, line_a.end, coordinate)
                                        == direction
                                        && orientation(line_b.start, line_b.end, coordinate)
                                            == direction
                                        && orientation(line_c.start, line_c.end, coordinate)
                                            == direction
                                    {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }

                false
            }
            Self::Sigma => {
                // true on loops
                for line in lines.iter() {
                    if !line.coordinates().contains(&coordinate) {
                        continue;
                    }

                    let mut frontier = vec![line.end];
                    let mut passed = Vec::<SigilCoordinate>::new();
                    while let Some(frontier_coordinate) = frontier.pop() {
                        if frontier_coordinate == line.start {
                            return true;
                        }
                        passed.push(frontier_coordinate);
                        'next: for next_line in lines.iter() {
                            if next_line.start != frontier_coordinate {
                                continue;
                            }

                            for passed_coordinate in passed.iter() {
                                if *passed_coordinate == next_line.end {
                                    continue 'next;
                                }
                            }

                            frontier.push(next_line.end);
                        }
                    }
                }
                false
            }
        }
    }

    pub fn texture_coordinate(&self) -> TextureCoordinate {
        let x = match self {
            Self::Alpha => 0.0,
            Self::Sigma => 0.25,
            Self::Delta => 0.5,
            Self::Phi => 0.75,
        };
        TextureCoordinate {
            width: 0.25,
            height: -1.0,
            y: 1.0,
            x,
        }
    }
}

#[derive(Copy, Debug, Clone, Serialize, Deserialize)]
pub struct Rune {
    pub sigil: Sigil,
    pub orb: Orb,
}

pub type CoordinateScalar = i32;
pub type Position = Vector2<f32>;
pub type SigilCoordinate = Vector2<CoordinateScalar>;

pub trait Coordinate {
    fn position(&self) -> Position;
    fn from_position(position: Position) -> Self;
}

impl Coordinate for SigilCoordinate {
    fn position(&self) -> Vector2<f32> {
        Vector2::new(
            self.x as f32 * SIGIL_DISTANCE,
            self.y as f32 * SIGIL_DISTANCE,
        )
    }

    fn from_position(position: Vector2<f32>) -> Self {
        Vector2::new(
            (position.x / SIGIL_DISTANCE + 0.5).floor() as CoordinateScalar,
            (position.y / SIGIL_DISTANCE + 0.5).floor() as CoordinateScalar,
        )
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub struct Line {
    pub start: SigilCoordinate,
    pub end: SigilCoordinate,
}

impl Line {
    /// returns if the other line branches or extends from this
    fn extends(&self, other: &Self) -> bool {
        let self_coordinates = self.coordinates();
        let mut coordinates = self_coordinates.iter();
        coordinates.next();

        let mut other_coordinates = other.coordinates();
        other_coordinates.pop();

        for coordinate in coordinates {
            for other_coordinate in other_coordinates.iter() {
                if coordinate == other_coordinate {
                    return true;
                }
            }
        }
        false
    }

    fn sprite(&self, time: f32) -> Sprite {
        let start = self.start.position();
        let end = self.end.position();
        let position = (start + end) / 2.0;
        let direction = end - start;
        let magnitude = direction.magnitude();

        let rotation = if direction.x < 0.0 {
            (direction.y / magnitude).asin() + PI
        } else {
            (-direction.y / magnitude).asin()
        };

        Sprite {
            transform: Transform {
                position,
                rotation,
                scale: Vector2::new(magnitude, SIGIL_SIZE),
            },
            texture_coordinate: TextureCoordinate {
                height: 1.0,
                x: -time,
                y: 0.0,
                width: magnitude / SIGIL_SIZE,
            },
            ..Default::default()
        }
    }

    fn intersects(&self, other: &Self) -> bool {
        // uses https://www.geeksforgeeks.org/check-if-two-given-line-segments-intersect/
        fn on_segment(a: Point, b: Point, c: Point) -> bool {
            b.x <= a.x.max(c.x) && b.x >= a.x.min(c.x) && b.y <= a.y.max(c.y) && b.y >= a.y.min(c.y)
        }

        // Find the four orientations needed for general and
        // special cases
        let o1 = orientation(self.start, self.end, other.start);
        let o2 = orientation(self.start, self.end, other.end);
        let o3 = orientation(other.start, other.end, self.start);
        let o4 = orientation(other.start, other.end, self.end);

        // can detatch by one point
        if (o1 == Orientation::Collinear && o2 != Orientation::Collinear)
            || (o2 == Orientation::Collinear && o1 != Orientation::Collinear)
        {
            return false;
        }

        if (o3 == Orientation::Collinear && o4 != Orientation::Collinear)
            || (o4 == Orientation::Collinear && o3 != Orientation::Collinear)
        {
            return false;
        }

        // can extend in same direction
        if o1 == Orientation::Collinear
            && o2 == Orientation::Collinear
            && (other.start.x.min(other.end.x) >= self.start.x.max(self.end.x)
                || other.start.x.max(other.end.x) <= self.start.x.min(self.end.x))
            && (other.start.y.min(other.end.y) >= self.start.y.max(self.end.y)
                || other.start.y.max(other.end.y) <= self.start.y.min(self.end.y))
        {
            return false;
        }

        if o2 == Orientation::Collinear
            && o3 == Orientation::Collinear
            && (self.start.x.min(self.end.x) >= other.start.x.max(other.end.x)
                || self.start.x.max(self.end.x) <= other.start.x.min(other.end.x))
            && (self.start.y.min(self.end.y) >= other.start.y.max(other.end.y)
                || self.start.y.max(self.end.y) <= other.start.y.min(other.end.y))
        {
            return false;
        }

        // General case
        if o1 != o2 && o3 != o4 {
            return true;
        }

        // Special Cas}
        // p1, q1 and p2 are collinear and p2 lies on segment p1q1
        if o1 == Orientation::Collinear && on_segment(self.start, other.start, self.end) {
            return true;
        }

        // p1, q1 and q2 are collinear and q2 lies on segment p1q1
        if o2 == Orientation::Collinear && on_segment(self.start, other.end, self.end) {
            return true;
        }

        // p2, q2 and p1 are collinear and p1 lies on segment p2q2
        if o3 == Orientation::Collinear && on_segment(other.start, self.start, other.end) {
            return true;
        }

        // p2, q2 and q1 are collinear and q1 lies on segment p2q2
        if o4 == Orientation::Collinear && on_segment(other.start, self.end, other.end) {
            return true;
        }

        false // Doesn't fall in any of the above cases
    }

    /// retrieves all touching coordinates
    fn coordinates(&self) -> Vec<SigilCoordinate> {
        if self.end == self.start {
            return vec![self.start];
        }
        let mut slope = self.end - self.start;
        let cardinal_direction = SigilCoordinate::new(
            if slope.x < 0 { -1 } else { 1 },
            if slope.y < 0 { -1 } else { 1 },
        );
        // for gcd
        slope.component_mul_assign(&cardinal_direction);

        // number of touching coordinates after start
        let gcd = (slope.x as u32).gcd(slope.y as u32) as i32;
        slope.component_mul_assign(&cardinal_direction);
        slope /= gcd;
        let mut coordinates = vec![self.start];
        for i in 1..=gcd {
            coordinates.push(slope * i + self.start);
        }
        coordinates
    }
}

#[derive(Clone, Debug)]
pub struct Puzzle {
    pub runes: HashMap<SigilCoordinate, Rune>,
    pub lines: Vec<Line>,
    pub cursor: SigilCoordinate,
}

impl Default for Puzzle {
    fn default() -> Self {
        Self {
            runes: HashMap::new(),
            lines: Vec::new(),
            cursor: Vector2::zeros(),
        }
    }
}

impl Puzzle {
    pub fn input(&mut self, coordinate: &SigilCoordinate) {
        if *coordinate == self.cursor {
            return;
        }
        if let Some(cursor_rune) = self.runes.get(&self.cursor) {
            let line = Line {
                start: self.cursor,
                end: *coordinate,
            };

            if !cursor_rune.orb.allow_intersections() && self.intersects_lines(&line) {
                return;
            }

            if let Some(_rune) = self.runes.get(coordinate) {
                cursor_rune
                    .orb
                    .effect(self.cursor, *coordinate, &mut self.lines);

                self.lines.push(line);
                self.cursor = *coordinate;
            }
        }
    }

    fn intersects_lines(&self, line: &Line) -> bool {
        for other_line in self.lines.iter() {
            if other_line.intersects(line) {
                return true;
            }
        }
        false
    }

    pub fn sprite_batches<'a>(
        &'a self,
        time: f32,
        cursor_texture: &'a wgpu::BindGroup,
        sigils_texture: &'a wgpu::BindGroup,
        orbs_texture: &'a wgpu::BindGroup,
        line_texture: &'a wgpu::BindGroup,
    ) -> Vec<SpriteBatch> {
        let mut orb_sprites = Vec::<Sprite>::new();
        let mut sigil_sprites = Vec::<Sprite>::new();

        for (coordinate, rune) in self.runes.iter() {
            let transform = Transform {
                position: coordinate.position(),
                rotation: 0.0,
                scale: SIGIL_SCALE,
            };
            let orb_coordinate = rune
                .orb
                .texture_coordinate(rune.sigil.active(*coordinate, &self.lines));

            orb_sprites.push(Sprite {
                transform,
                texture_coordinate: orb_coordinate,
                ..Default::default()
            });

            sigil_sprites.push(Sprite {
                transform,
                texture_coordinate: rune.sigil.texture_coordinate(),
                ..Default::default()
            });
        }

        let lines = SpriteBatch {
            sprites: self.lines.iter().map(|line| line.sprite(time)).collect(),
            texture_bind_group: line_texture,
        };

        let cursor = SpriteBatch {
            sprites: vec![Sprite {
                transform: Transform {
                    scale: Vector2::new(CURSOR_SIZE, CURSOR_SIZE),
                    position: self.cursor.position(),
                    rotation: 0.0,
                },
                ..Default::default()
            }],
            texture_bind_group: cursor_texture,
        };

        vec![
            lines,
            cursor,
            SpriteBatch {
                sprites: orb_sprites,
                texture_bind_group: orbs_texture,
            },
            SpriteBatch {
                sprites: sigil_sprites,
                texture_bind_group: sigils_texture,
            },
        ]
    }
}

#[derive(Serialize, Deserialize)]
pub struct SerialablePuzzle {
    runes: HashMap<String, Rune>,
    lines: Vec<Line>,
    cursor: SigilCoordinate,
}

impl From<Puzzle> for SerialablePuzzle {
    fn from(mut value: Puzzle) -> Self {
        let mut runes = HashMap::<String, Rune>::new();
        for (coordinate, rune) in value.runes.drain() {
            runes.insert(format!("{} {}", coordinate.x, coordinate.y), rune);
        }

        Self {
            runes,
            lines: value.lines,
            cursor: value.cursor,
        }
    }
}

#[derive(Debug)]
pub enum ConvertSeriablePuzzleError {
    ParseInt(ParseIntError),
    NoString,
    NoYCoordinate,
}

fn map_seriable_error(err: ParseIntError) -> ConvertSeriablePuzzleError {
    ConvertSeriablePuzzleError::ParseInt(err)
}

impl TryFrom<SerialablePuzzle> for Puzzle {
    type Error = ConvertSeriablePuzzleError;
    fn try_from(mut value: SerialablePuzzle) -> Result<Self, Self::Error> {
        let mut runes = HashMap::<SigilCoordinate, Rune>::new();

        for (coordinate, rune) in value.runes.drain() {
            let mut strings = coordinate.split(' ');
            if let Some(first) = strings.next() {
                if let Some(second) = strings.next() {
                    runes.insert(
                        SigilCoordinate::new(
                            first.parse::<i32>().map_err(map_seriable_error)?,
                            second.parse::<i32>().map_err(map_seriable_error)?,
                        ),
                        rune,
                    );
                } else {
                    return Err(ConvertSeriablePuzzleError::NoYCoordinate);
                }
            } else {
                return Err(ConvertSeriablePuzzleError::NoString);
            }
        }

        Ok(Self {
            runes,
            lines: value.lines,
            cursor: value.cursor,
        })
    }
}

type Point = Vector2<i32>;
#[derive(Eq, PartialEq)]
enum Orientation {
    Collinear,
    Clockwise,
    CounterClockwise,
}

fn orientation(a: Point, b: Point, c: Point) -> Orientation {
    match (b.y - a.y) * (c.x - b.x) - (b.x - a.x) * (c.y - b.y) {
        val if val == 0 => Orientation::Collinear,
        val if val > 0 => Orientation::Clockwise,
        _ => Orientation::CounterClockwise,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn puzzle_can_serialize() {
        let runes = HashMap::<SigilCoordinate, Rune>::new();
        let puzzle = Puzzle {
            runes,
            lines: vec![],
            cursor: SigilCoordinate::zeros(),
        };
        let serialized = SerialablePuzzle::from(puzzle);
        Puzzle::try_from(serialized).unwrap();
    }
}
