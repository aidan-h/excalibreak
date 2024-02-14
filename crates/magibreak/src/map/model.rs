use excali_3d::*;
use wgpu::util::DeviceExt;
use wgpu::Device;

use crate::map::grid::CHUNK_SIZE;

use super::grid::Grid;

pub fn from_marching_squares(device: &Device, grid: &Grid) -> Model {
    /// create 2 vertices below first two and a quad at quad_vertex
    #[allow(clippy::too_many_arguments)]
    fn drop_line(
        mut left: [f32; 3],
        left_vertex: u16,
        mut right: [f32; 3],
        right_vertex: u16,
        quad_vertex: u16,
        color: &[f32; 3],
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
    ) {
        left[1] -= 1.0;
        right[1] -= 1.0;
        vertices.push(Vertex::new(left, *color));
        vertices.push(Vertex::new(right, *color));
        indices.push(left_vertex);
        indices.push(right_vertex);
        indices.push(quad_vertex);

        indices.push(right_vertex);
        indices.push(quad_vertex + 1);
        indices.push(quad_vertex);
    }

    #[allow(clippy::too_many_arguments)]
    fn bottom_left(
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        x: f32,
        y: f32,
        height: f32,
        vertex: u16,
        color: &[f32; 3],
        drop_color: &[f32; 3],
    ) {
        let left = [x + 0.5, height, y];
        let right = [x, height, y + 0.5];
        vertices.push(Vertex::new([x, height, y], *color));
        vertices.push(Vertex::new(left, *color));
        vertices.push(Vertex::new(right, *color));
        indices.push(vertex + 1);
        indices.push(vertex);
        indices.push(vertex + 2);
        drop_line(
            left,
            vertex + 1,
            right,
            vertex + 2,
            vertex + 3,
            drop_color,
            vertices,
            indices,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn bottom_right(
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        x: f32,
        y: f32,
        height: f32,
        vertex: u16,
        color: &[f32; 3],
        drop_color: &[f32; 3],
    ) {
        let left = [x + 1.0, height, y + 0.5];
        let right = [x + 0.5, height, y];
        vertices.push(Vertex::new([x + 1.0, height, y], *color));
        vertices.push(Vertex::new(left, *color));
        vertices.push(Vertex::new(right, *color));
        indices.push(vertex + 1);
        indices.push(vertex);
        indices.push(vertex + 2);
        drop_line(
            left,
            vertex + 1,
            right,
            vertex + 2,
            vertex + 3,
            drop_color,
            vertices,
            indices,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn top_right(
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        x: f32,
        y: f32,
        height: f32,
        vertex: u16,
        color: &[f32; 3],
        drop_color: &[f32; 3],
    ) {
        let left = [x + 0.5, height, y + 1.0];
        let right = [x + 1.0, height, y + 0.5];
        vertices.push(Vertex::new([x + 1.0, height, y + 1.0], *color));
        vertices.push(Vertex::new(left, *color));
        vertices.push(Vertex::new(right, *color));
        indices.push(vertex + 2);
        indices.push(vertex + 1);
        indices.push(vertex);
        drop_line(
            left,
            vertex + 1,
            right,
            vertex + 2,
            vertex + 3,
            drop_color,
            vertices,
            indices,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn top_left(
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        x: f32,
        y: f32,
        height: f32,
        vertex: u16,
        color: &[f32; 3],
        drop_color: &[f32; 3],
    ) {
        let left = [x, height, y + 0.5];
        let right = [x + 0.5, height, y + 1.0];
        vertices.push(Vertex::new([x, height, y + 1.0], *color));
        vertices.push(Vertex::new(left, *color));
        vertices.push(Vertex::new(right, *color));
        indices.push(vertex + 2);
        indices.push(vertex + 1);
        indices.push(vertex);
        drop_line(
            left,
            vertex + 1,
            right,
            vertex + 2,
            vertex + 3,
            drop_color,
            vertices,
            indices,
        );
    }

    fn add_l_indices(indices: &mut Vec<u16>, vertex: u16) {
        indices.push(vertex);
        indices.push(vertex + 1);
        indices.push(vertex + 2);

        indices.push(vertex);
        indices.push(vertex + 2);
        indices.push(vertex + 3);

        indices.push(vertex);
        indices.push(vertex + 3);
        indices.push(vertex + 4);
    }
    let mut vertices = Vec::<Vertex>::new();
    let mut indices = Vec::<u16>::new();
    let mut min = 0u16;
    let mut max = 0u16;

    for y in 0..CHUNK_SIZE - 1 {
        for x in 0..CHUNK_SIZE - 1 {
            let height = grid.height_map.row(y)[x];
            min = min.min(height);
            max = max.max(height);
        }
    }

    for threshold in min..=max {
        let value = threshold as f32 / max as f32;
        let color = &[value, value, value];

        let drop_value = (threshold as f32 - 1.0) / max as f32;
        let drop_color = &[drop_value, drop_value, drop_value];

        for y in 0..CHUNK_SIZE - 1 {
            for x in 0..CHUNK_SIZE - 1 {
                let vertex = vertices.len() as u16;

                let mut case = 0;

                let bl = grid.height_map.row(y)[x];
                let br = grid.height_map.row(y)[x + 1];
                let tl = grid.height_map.row(y + 1)[x];
                let tr = grid.height_map.row(y + 1)[x + 1];

                // no need to render lower flat surfaces underneath case 15s
                if bl > threshold && br > threshold && tl > threshold && tr > threshold {
                    continue;
                }

                if bl >= threshold {
                    case += 1;
                }
                if br >= threshold {
                    case += 2;
                }
                if tr >= threshold {
                    case += 4;
                }
                if tl >= threshold {
                    case += 8;
                }
                let height = threshold as f32;
                let x = x as f32;
                let y = y as f32;
                const CORNER_VERTICES: u16 = 5;
                match case {
                    1 => bottom_left(
                        &mut vertices,
                        &mut indices,
                        x,
                        y,
                        height,
                        vertex,
                        color,
                        drop_color,
                    ),
                    2 => bottom_right(
                        &mut vertices,
                        &mut indices,
                        x,
                        y,
                        height,
                        vertex,
                        color,
                        drop_color,
                    ),
                    3 => {
                        let left = [x + 1.0, height, y + 0.5];
                        let right = [x, height, y + 0.5];
                        vertices.push(Vertex::new([x, height, y], *color));
                        vertices.push(Vertex::new([x + 1.0, height, y], *color));
                        vertices.push(Vertex::new(left, *color));
                        vertices.push(Vertex::new(right, *color));

                        indices.push(vertex + 2);
                        indices.push(vertex + 1);
                        indices.push(vertex);

                        indices.push(vertex + 3);
                        indices.push(vertex + 2);
                        indices.push(vertex);
                        drop_line(
                            left,
                            vertex + 2,
                            right,
                            vertex + 3,
                            vertex + 4,
                            drop_color,
                            &mut vertices,
                            &mut indices,
                        );
                    }
                    4 => top_right(
                        &mut vertices,
                        &mut indices,
                        x,
                        y,
                        height,
                        vertex,
                        color,
                        drop_color,
                    ),
                    5 => {
                        bottom_left(
                            &mut vertices,
                            &mut indices,
                            x,
                            y,
                            height,
                            vertex,
                            color,
                            drop_color,
                        );
                        top_right(
                            &mut vertices,
                            &mut indices,
                            x,
                            y,
                            height,
                            vertex + CORNER_VERTICES,
                            color,
                            drop_color,
                        );
                    }
                    6 => {
                        //XO
                        //XO
                        let left = [x + 0.5, height, y + 1.0];
                        let right = [x + 0.5, height, y];
                        vertices.push(Vertex::new([x + 1.0, height, y], *color));
                        vertices.push(Vertex::new([x + 1.0, height, y + 1.0], *color));
                        vertices.push(Vertex::new(left, *color));
                        vertices.push(Vertex::new(right, *color));

                        indices.push(vertex + 2);
                        indices.push(vertex + 1);
                        indices.push(vertex);

                        indices.push(vertex + 3);
                        indices.push(vertex + 2);
                        indices.push(vertex);
                        drop_line(
                            left,
                            vertex + 2,
                            right,
                            vertex + 3,
                            vertex + 4,
                            drop_color,
                            &mut vertices,
                            &mut indices,
                        );
                    }
                    7 => {
                        //XO
                        //OO
                        let left = [x + 0.5, height, y + 1.0];
                        let right = [x, height, y + 0.5];
                        vertices.push(Vertex::new([x, height, y], *color));
                        vertices.push(Vertex::new(right, *color));
                        vertices.push(Vertex::new(left, *color));
                        vertices.push(Vertex::new([x + 1.0, height, y + 1.0], *color));
                        vertices.push(Vertex::new([x + 1.0, height, y], *color));
                        add_l_indices(&mut indices, vertex);
                        drop_line(
                            left,
                            vertex + 2,
                            right,
                            vertex + 1,
                            vertex + 5,
                            drop_color,
                            &mut vertices,
                            &mut indices,
                        );
                    }
                    8 => top_left(
                        &mut vertices,
                        &mut indices,
                        x,
                        y,
                        height,
                        vertex,
                        color,
                        drop_color,
                    ),
                    9 => {
                        let left = [x + 0.5, height, y];
                        let right = [x + 0.5, height, y + 1.0];
                        vertices.push(Vertex::new([x, height, y + 1.0], *color));
                        vertices.push(Vertex::new([x, height, y], *color));
                        vertices.push(Vertex::new(left, *color));
                        vertices.push(Vertex::new(right, *color));

                        indices.push(vertex + 2);
                        indices.push(vertex + 1);
                        indices.push(vertex);

                        indices.push(vertex + 3);
                        indices.push(vertex + 2);
                        indices.push(vertex);
                        drop_line(
                            left,
                            vertex + 2,
                            right,
                            vertex + 3,
                            vertex + 4,
                            drop_color,
                            &mut vertices,
                            &mut indices,
                        );
                    }
                    10 => {
                        //OX
                        //XO
                        top_left(
                            &mut vertices,
                            &mut indices,
                            x,
                            y,
                            height,
                            vertex,
                            color,
                            drop_color,
                        );
                        bottom_right(
                            &mut vertices,
                            &mut indices,
                            x,
                            y,
                            height,
                            vertex + CORNER_VERTICES,
                            color,
                            drop_color,
                        );
                    }
                    11 => {
                        //OX
                        //OO
                        let left = [x + 1.0, height, y + 0.5];
                        let right = [x + 0.5, height, y + 1.0];
                        vertices.push(Vertex::new([x, height, y], *color));
                        vertices.push(Vertex::new([x, height, y + 1.0], *color));
                        vertices.push(Vertex::new(right, *color));
                        vertices.push(Vertex::new(left, *color));
                        vertices.push(Vertex::new([x + 1.0, height, y], *color));

                        add_l_indices(&mut indices, vertex);
                        drop_line(
                            left,
                            vertex + 3,
                            right,
                            vertex + 2,
                            vertex + 5,
                            drop_color,
                            &mut vertices,
                            &mut indices,
                        );
                    }
                    12 => {
                        //OO
                        //XX
                        let left = [x, height, y + 0.5];
                        let right = [x + 1.0, height, y + 0.5];
                        vertices.push(Vertex::new([x, height, y + 1.0], *color));
                        vertices.push(Vertex::new([x + 1.0, height, y + 1.0], *color));
                        vertices.push(Vertex::new(left, *color));
                        vertices.push(Vertex::new(right, *color));

                        indices.push(vertex);
                        indices.push(vertex + 1);
                        indices.push(vertex + 3);

                        indices.push(vertex);
                        indices.push(vertex + 3);
                        indices.push(vertex + 2);
                        drop_line(
                            left,
                            vertex + 2,
                            right,
                            vertex + 3,
                            vertex + 4,
                            drop_color,
                            &mut vertices,
                            &mut indices,
                        );
                    }
                    13 => {
                        //OO
                        //OX
                        let left = [x + 0.5, height, y];
                        let right = [x + 1.0, height, y + 0.5];
                        vertices.push(Vertex::new([x, height, y], *color));
                        vertices.push(Vertex::new([x, height, y + 1.0], *color));
                        vertices.push(Vertex::new([x + 1.0, height, y + 1.0], *color));
                        vertices.push(Vertex::new(right, *color));
                        vertices.push(Vertex::new(left, *color));

                        add_l_indices(&mut indices, vertex);
                        drop_line(
                            left,
                            vertex + 4,
                            right,
                            vertex + 3,
                            vertex + 5,
                            drop_color,
                            &mut vertices,
                            &mut indices,
                        );
                    }
                    14 => {
                        //OO
                        //XO
                        let left = [x, height, y + 0.5];
                        let right = [x + 0.5, height, y];
                        vertices.push(Vertex::new(left, *color));
                        vertices.push(Vertex::new([x, height, y + 1.0], *color));
                        vertices.push(Vertex::new([x + 1.0, height, y + 1.0], *color));
                        vertices.push(Vertex::new([x + 1.0, height, y], *color));
                        vertices.push(Vertex::new(right, *color));

                        add_l_indices(&mut indices, vertex);
                        drop_line(
                            left,
                            vertex,
                            right,
                            vertex + 4,
                            vertex + 5,
                            drop_color,
                            &mut vertices,
                            &mut indices,
                        );
                    }
                    15 => {
                        vertices.push(Vertex::new([x, height, y], *color));
                        vertices.push(Vertex::new([x + 1.0, height, y], *color));
                        vertices.push(Vertex::new([x + 1.0, height, y + 1.0], *color));
                        vertices.push(Vertex::new([x, height, y + 1.0], *color));
                        indices.push(vertex);
                        indices.push(vertex + 3);
                        indices.push(vertex + 1);

                        indices.push(vertex + 3);
                        indices.push(vertex + 2);
                        indices.push(vertex + 1);
                    }
                    _ => {}
                }
            }
        }
    }

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Map Vertex Buffer"),
        contents: bytemuck::cast_slice(vertices.as_slice()),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Map Index Buffer"),
        contents: bytemuck::cast_slice(indices.as_slice()),
        usage: wgpu::BufferUsages::INDEX,
    });
    let indices = indices.len() as u32;

    Model {
        vertex_buffer,
        index_buffer,
        indices,
    }
}
