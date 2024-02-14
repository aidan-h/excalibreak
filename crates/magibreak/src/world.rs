use excali_3d::{Camera, FPSEye, LineRenderer, Renderer3D, Vertex};
use excali_input::Input;
use excali_render::Renderer;
use log::warn;
use nalgebra::{Vector2, Vector3};
use rapier3d::control::KinematicCharacterController;
use rapier3d::prelude::{
    BroadPhase, CCDSolver, ColliderBuilder, ColliderHandle, ColliderSet, DebugRenderBackend,
    DebugRenderObject, DebugRenderPipeline, ImpulseJointSet, IntegrationParameters, IslandManager,
    MultibodyJointSet, NarrowPhase, PhysicsPipeline, Point, QueryFilter, QueryPipeline, Real,
    RigidBodyBuilder, RigidBodyHandle, RigidBodySet,
};

use crate::input;

struct Character {
    controller: KinematicCharacterController,
    rigid_body: RigidBodyHandle,
    collider: ColliderHandle,
}

impl Character {
    fn new(rigid_bodies: &mut RigidBodySet, colliders: &mut ColliderSet) -> Self {
        let rigid_body = rigid_bodies.insert(
            RigidBodyBuilder::kinematic_position_based().translation(Vector3::new(0.0, 100.0, 0.0)),
        );
        Self {
            controller: Default::default(),
            collider: colliders.insert_with_parent(
                ColliderBuilder::capsule_y(0.3, 2.0),
                rigid_body,
                rigid_bodies,
            ),
            rigid_body,
        }
    }

    fn position(&self, engine: &PhysicsEngine) -> Option<Vector3<f32>> {
        Some(
            engine
                .bodies
                .get(self.rigid_body)?
                .position()
                .translation
                .vector,
        )
    }

    fn update(&mut self, translation: Vector3<f32>, engine: &mut PhysicsEngine, delta: f32) {
        if let Some(collider) = engine.colliders.get(self.collider) {
            let mut collisions = Vec::new();
            let filter = QueryFilter::default().exclude_rigid_body(self.rigid_body);
            let movement = self.controller.move_shape(
                delta,
                &engine.bodies,
                &engine.colliders,
                &engine.query_pipeline,
                collider.shape(),
                collider.position(),
                translation,
                filter,
                |collision| collisions.push(collision),
            );
            for collision in collisions {
                self.controller.solve_character_collision_impulses(
                    delta,
                    &mut engine.bodies,
                    &engine.colliders,
                    &engine.query_pipeline,
                    collider.shape(),
                    collider.mass(),
                    &collision,
                    filter,
                );
            }
            if let Some(rigid_body) = engine.bodies.get_mut(self.rigid_body) {
                rigid_body.set_next_kinematic_position(
                    (rigid_body.position().translation.vector + movement.translation).into(),
                );
                return;
            }
            warn!("Character's rigid body is missing");
            return;
        }
        warn!("Character's collider is missing");
    }
}

struct PhysicsEngine {
    physics_pipeline: PhysicsPipeline,
    query_pipeline: QueryPipeline,
    colliders: ColliderSet,
    bodies: RigidBodySet,
    integration_parameters: IntegrationParameters,
    islands: IslandManager,
    broad_phase: BroadPhase,
    narrow_phase: NarrowPhase,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    ccd_solver: CCDSolver,
    gravity: Vector3<f32>,
}

impl Default for PhysicsEngine {
    fn default() -> Self {
        Self {
            gravity: Vector3::new(0.0, -1.0, 0.0),
            physics_pipeline: Default::default(),
            query_pipeline: Default::default(),
            colliders: Default::default(),
            bodies: Default::default(),
            integration_parameters: Default::default(),
            islands: Default::default(),
            broad_phase: Default::default(),
            narrow_phase: Default::default(),
            impulse_joints: Default::default(),
            multibody_joints: Default::default(),
            ccd_solver: Default::default(),
        }
    }
}

/// The entire 3D space of the game
pub struct World {
    physics_engine: PhysicsEngine,
    character: Character,
    renderer: Renderer3D,
    line_renderer: LineRenderer,
    camera: Camera<FPSEye>,
}

#[derive(Default)]
struct DebugPhysicsRenderer {
    vertices: Vec<Vertex>,
}
impl DebugRenderBackend for DebugPhysicsRenderer {
    fn draw_line(
        &mut self,
        _object: DebugRenderObject<'_>,
        a: Point<Real>,
        b: Point<Real>,
        color: [f32; 4],
    ) {
        self.vertices
            .push(Vertex::new(a.into(), [color[0], color[1], color[2]]));
        self.vertices
            .push(Vertex::new(b.into(), [color[0], color[1], color[2]]));
    }
}

impl PhysicsEngine {
    fn draw(
        &self,
        view: &wgpu::TextureView,
        line_renderer: &mut LineRenderer,
        renderer_3d: &Renderer3D,
        renderer: &Renderer,
    ) -> wgpu::CommandBuffer {
        let mut debug_renderer = DebugPhysicsRenderer::default();
        DebugRenderPipeline::default().render_colliders(
            &mut debug_renderer,
            &self.bodies,
            &self.colliders,
        );
        line_renderer.draw(debug_renderer.vertices, renderer, renderer_3d, view)
    }

    fn step(&mut self) {
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.ccd_solver,
            Some(&mut self.query_pipeline),
            &(),
            &(),
        );
    }
}

impl World {
    pub fn new(renderer: &Renderer) -> Self {
        let mut physics_engine = PhysicsEngine::default();
        physics_engine
            .colliders
            .insert(ColliderBuilder::cuboid(10.0, 1.0, 10.0).build());

        let renderer_3d = Renderer3D::new(&renderer.config, &renderer.device, 10);
        let camera = Camera {
            position: Vector3::new(0.0, 3.0, -10.0).into(),
            ..Default::default()
        };
        Self {
            character: Character::new(&mut physics_engine.bodies, &mut physics_engine.colliders),
            physics_engine,
            line_renderer: LineRenderer::new(renderer, &renderer_3d, 10),
            camera,
            renderer: renderer_3d,
        }
    }

    pub fn update(
        &mut self,
        renderer: &Renderer,
        view: &wgpu::TextureView,
        delta: f32,
        input: &Input<input::Actions>,
    ) -> wgpu::CommandBuffer {
        const SPEED: f32 = 0.5;
        const CAMERA_SENSITIVITY: f32 = 0.02;

        self.camera.aspect = renderer.aspect_ratio();
        let mut direction = Vector3::<f32>::zeros();

        if input.input_map.camera_forward.button.pressed() {
            direction.z += 1.0;
        }
        if input.input_map.camera_backward.button.pressed() {
            direction.z -= 1.0;
        }
        if input.input_map.camera_right.button.pressed() {
            direction.x -= 1.0;
        }
        if input.input_map.camera_left.button.pressed() {
            direction.x += 1.0;
        }
        if input.input_map.camera_down.button.pressed() {
            direction.y -= 1.0;
        }
        if input.input_map.camera_up.button.pressed() {
            direction.y += 1.0;
        }

        if let Some(mouse_delta) = input.mouse_delta {
            if input.mouse_locked() {
                self.camera.rotate(
                    &Vector2::new(mouse_delta.0.x as f32, mouse_delta.0.y as f32),
                    CAMERA_SENSITIVITY,
                );
            }
        }

        self.character.update(
            self.physics_engine.gravity
                + (self.camera.point_to_world_space(&direction) - self.camera.position.coords)
                    * SPEED,
            &mut self.physics_engine,
            delta,
        );
        self.physics_engine.step();

        if let Some(position) = self.character.position(&self.physics_engine) {
            self.camera.position = position.into();
        }

        self.renderer.update_camera(&self.camera, renderer);
        self.physics_engine
            .draw(view, &mut self.line_renderer, &self.renderer, renderer)
    }
}
