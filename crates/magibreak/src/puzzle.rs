use excali_io::{FromKeyError, SerializeKey};
use excali_ui::Mode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f32::consts::PI;

use excali_sprite::*;
use gcd::Gcd;
use nalgebra::Vector2;

const SIGIL_DISTANCE: f32 = 23.0;
const LINE_WIDTH: f32 = 19.0;

pub trait FromSigilCoordinate {
    fn from_sigil_coordinate(coordinate: SigilCoordinate, camera: &Transform) -> Self;
}

impl FromSigilCoordinate for Transform {
    fn from_sigil_coordinate(coordinate: SigilCoordinate, camera: &Transform) -> Self {
        camera * &Transform::from_position(coordinate.position())
    }
}

#[derive(Serialize, Debug, PartialEq, Eq, Deserialize, Copy, Clone)]
pub enum Orb {
    Circle,
    Diamond,
    Octogon,
}

impl Mode for Orb {
    fn change(&self) -> Self {
        match *self {
            Self::Circle => Self::Diamond,
            Self::Diamond => Self::Octogon,
            Self::Octogon => Self::Circle,
        }
    }
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
            Self::Octogon => 0.5,
            Self::Diamond => 0.25,
            Self::Circle => 0.0,
        };
        TextureCoordinate {
            width: 0.5,
            height: 0.25,
            x,
            y,
        }
    }
}

#[derive(Serialize, Debug, Deserialize, Copy, Clone)]
pub enum Rune {
    Alpha,
    Sigma,
    Delta,
    Phi,
}

impl ToString for Rune {
    fn to_string(&self) -> String {
        match self {
            Self::Alpha => "Alpha".to_string(),
            Self::Sigma => "Sigma".to_string(),
            Self::Delta => "Delta".to_string(),
            Self::Phi => "Phi".to_string(),
        }
    }
}

impl Mode for Rune {
    fn change(&self) -> Self {
        match *self {
            Self::Alpha => Self::Delta,
            Self::Delta => Self::Phi,
            Self::Phi => Self::Sigma,
            Self::Sigma => Self::Alpha,
        }
    }
}

impl Rune {
    fn active<T>(
        &self,
        coordinate: SigilCoordinate,
        lines: &[Line],
        sigils: &HashMap<SigilCoordinate, T>,
    ) -> bool {
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
                        if line_a.extends(line_b, sigils) {
                            for line_c in lines.iter() {
                                if line_b.extends(line_c, sigils) && line_c.extends(line_a, sigils)
                                {
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
pub struct Sigil {
    pub rune: Rune,
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
    /// returns if the other line branches or extends from this and there is a sigil at the
    /// intersection
    fn extends<T>(&self, other: &Self, sigils: &HashMap<SigilCoordinate, T>) -> bool {
        let self_coordinates = self.coordinates();
        let mut coordinates = self_coordinates.iter();
        coordinates.next();

        let mut other_coordinates = other.coordinates();
        other_coordinates.pop();

        for coordinate in coordinates {
            if sigils.get(coordinate).is_none() {
                continue;
            }
            for other_coordinate in other_coordinates.iter() {
                if coordinate == other_coordinate {
                    return true;
                }
            }
        }
        false
    }

    fn sprite(&self, time: f32, camera: &Transform) -> Sprite {
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
            transform: camera
                * &Transform {
                    position,
                    rotation,
                    scale: Vector2::new(1.0, 1.0),
                },
            texture_coordinate: TextureCoordinate {
                height: 1.0,
                x: -time,
                y: 0.0,
                width: magnitude / LINE_WIDTH,
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

/// The puzzle which the player interacts with
#[derive(Debug)]
pub struct ActivePuzzle {
    puzzle: Puzzle,
    history: Vec<Puzzle>,
}

impl ActivePuzzle {
    pub fn new(puzzle: Puzzle) -> Self {
        Self {
            puzzle,
            history: Vec::new(),
        }
    }

    pub fn load_puzzle(&mut self, puzzle: Puzzle) {
        self.puzzle = puzzle;
        self.history.clear();
    }

    pub fn undo(&mut self) -> bool {
        if let Some(new_puzzle) = self.history.pop() {
            self.puzzle = new_puzzle;
            return true;
        }
        false
    }

    pub fn input(&mut self, coordinate: &SigilCoordinate) {
        let past = self.puzzle.clone();
        if self.puzzle.input(coordinate) {
            self.history.push(past);
        }
    }

    pub fn sprite_batches<'a>(
        &'a self,
        time: f32,
        camera: &Transform,
        cursor_texture: &'a SpriteTexture,
        sigils_texture: &'a SpriteTexture,
        orbs_texture: &'a SpriteTexture,
        line_texture: &'a SpriteTexture,
    ) -> Vec<SpriteBatch> {
        self.puzzle.sprite_batches(
            time,
            camera,
            cursor_texture,
            sigils_texture,
            orbs_texture,
            line_texture,
        )
    }
}

#[derive(Clone, Debug)]
pub struct Puzzle {
    pub sigils: HashMap<SigilCoordinate, Sigil>,
    pub lines: Vec<Line>,
    pub cursor: SigilCoordinate,
}

impl Default for Puzzle {
    fn default() -> Self {
        Self {
            sigils: HashMap::new(),
            lines: Vec::new(),
            cursor: Vector2::zeros(),
        }
    }
}

impl Puzzle {
    /// returns if the input does anything
    pub fn input(&mut self, coordinate: &SigilCoordinate) -> bool {
        if *coordinate == self.cursor {
            return false;
        }
        if let Some(cursor_rune) = self.sigils.get(&self.cursor) {
            let line = Line {
                start: self.cursor,
                end: *coordinate,
            };

            if !cursor_rune.orb.allow_intersections() && self.intersects_lines(&line) {
                return false;
            }

            if self.sigils.get(coordinate).is_some() {
                cursor_rune
                    .orb
                    .effect(self.cursor, *coordinate, &mut self.lines);

                self.lines.push(line);
                self.cursor = *coordinate;
                return true;
            }
        }
        false
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
        camera: &Transform,
        cursor_texture: &'a SpriteTexture,
        sigils_texture: &'a SpriteTexture,
        orbs_texture: &'a SpriteTexture,
        line_texture: &'a SpriteTexture,
    ) -> Vec<SpriteBatch> {
        let mut orb_sprites = Vec::<Sprite>::new();
        let mut sigil_sprites = Vec::<Sprite>::new();

        for (coordinate, rune) in self.sigils.iter() {
            let transform = camera * &Transform::from_position(coordinate.position());
            let orb_coordinate = rune.orb.texture_coordinate(rune.rune.active(
                *coordinate,
                &self.lines,
                &self.sigils,
            ));

            orb_sprites.push(Sprite {
                transform,
                texture_coordinate: orb_coordinate,
                ..Default::default()
            });

            sigil_sprites.push(Sprite {
                transform,
                texture_coordinate: rune.rune.texture_coordinate(),
                ..Default::default()
            });
        }

        let lines = SpriteBatch {
            sprites: self
                .lines
                .iter()
                .map(|line| line.sprite(time, camera))
                .collect(),
            texture: line_texture,
        };

        let cursor = SpriteBatch {
            sprites: vec![Sprite {
                transform: camera * &Transform::from_position(self.cursor.position()),
                ..Default::default()
            }],
            texture: cursor_texture,
        };

        vec![
            lines,
            cursor,
            SpriteBatch {
                sprites: orb_sprites,
                texture: orbs_texture,
            },
            SpriteBatch {
                sprites: sigil_sprites,
                texture: sigils_texture,
            },
        ]
    }
}

#[derive(Serialize, Deserialize)]
pub struct SerialablePuzzle {
    sigils: HashMap<String, Sigil>,
    lines: Vec<Line>,
    cursor: SigilCoordinate,
}

impl From<Puzzle> for SerialablePuzzle {
    fn from(value: Puzzle) -> Self {
        Self {
            sigils: SigilCoordinate::serialize_hash_map(&value.sigils),
            lines: value.lines,
            cursor: value.cursor,
        }
    }
}

impl TryFrom<SerialablePuzzle> for Puzzle {
    type Error = FromKeyError<i32>;
    fn try_from(value: SerialablePuzzle) -> Result<Self, Self::Error> {
        Ok(Self {
            sigils: Vector2::<i32>::deserialize_hash_map(&value.sigils)?,
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
        let runes = HashMap::<SigilCoordinate, Sigil>::new();
        let puzzle = Puzzle {
            sigils: runes,
            lines: vec![],
            cursor: SigilCoordinate::zeros(),
        };
        let serialized = SerialablePuzzle::from(puzzle);
        Puzzle::try_from(serialized).unwrap();
    }
}
