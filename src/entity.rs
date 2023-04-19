use crate::script::Script;
use crate::utils;
use crate::world::{self, Cell, CellPos, Point, WorldPos, AABB};
use array2d::Array2D;
use sdl2::rect::Rect;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default)]
pub enum Direction {
    Up,
    #[default]
    Down,
    Left,
    Right,
}

#[derive(Clone, Debug, Default)]
pub struct Entity {
    pub id: String,
    pub position: RefCell<Option<WorldPos>>,
    pub sprite_component: RefCell<Option<SpriteComponent>>,
    pub facing: RefCell<Option<Direction>>,
    pub walking_component: RefCell<Option<WalkingComponent>>,
    pub collision_component: RefCell<Option<CollisionComponent>>,
    pub scripts: RefCell<Option<Vec<Script>>>,
}

#[derive(Clone, Debug)]
pub struct SpriteComponent {
    pub spriteset_rect: Rect, // The region of the full spritesheet with this entity's sprites
    pub sprite_offset: Point<i32>,
    pub sine_offset_animation: Option<SineOffsetAnimation>,

    // TODO: ad hoc
    pub dead_sprite: Option<Rect>,
}

#[derive(Clone, Debug)]
pub struct SineOffsetAnimation {
    pub start_time: Instant,
    pub duration: Duration,
    pub amplitude: f64,
    pub frequency: f64,
    pub direction: Point<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct WalkingComponent {
    pub speed: f64,
    pub direction: Direction,
    pub destination: Option<WorldPos>,
}

#[derive(Clone, Debug)]
pub struct CollisionComponent {
    pub hitbox_dimensions: Point<f64>,
    pub solid: bool,
}

pub fn walk_and_resolve_collisions(
    id: &String,
    position: &mut WorldPos,
    walking_component: &WalkingComponent,
    collision_component: &CollisionComponent,
    tilemap: &Array2D<Cell>,
    entities: &HashMap<String, Entity>,
) {
    let new_position = *position
        + match walking_component.direction {
            Direction::Up => WorldPos::new(0.0, -walking_component.speed),
            Direction::Down => WorldPos::new(0.0, walking_component.speed),
            Direction::Left => WorldPos::new(-walking_component.speed, 0.0),
            Direction::Right => WorldPos::new(walking_component.speed, 0.0),
        };

    let old_aabb = AABB::from_pos_and_hitbox(*position, collision_component.hitbox_dimensions);

    let mut new_aabb =
        AABB::from_pos_and_hitbox(new_position, collision_component.hitbox_dimensions);

    // Check for and resolve collision with the 9 cells centered around new position
    let new_cellpos = new_position.to_cellpos();
    let cellposes_to_check = [
        CellPos::new(new_cellpos.x - 1, new_cellpos.y - 1),
        CellPos::new(new_cellpos.x, new_cellpos.y - 1),
        CellPos::new(new_cellpos.x + 1, new_cellpos.y - 1),
        CellPos::new(new_cellpos.x - 1, new_cellpos.y),
        CellPos::new(new_cellpos.x, new_cellpos.y),
        CellPos::new(new_cellpos.x + 1, new_cellpos.y),
        CellPos::new(new_cellpos.x - 1, new_cellpos.y + 1),
        CellPos::new(new_cellpos.x, new_cellpos.y + 1),
        CellPos::new(new_cellpos.x + 1, new_cellpos.y + 1),
    ];
    for cellpos in cellposes_to_check {
        if let Some(cell) = world::get_cell_at_cellpos(tilemap, cellpos) {
            if !cell.passable {
                let cell_aabb =
                    AABB::from_pos_and_hitbox(cellpos.to_worldpos(), Point::new(1., 1.));
                new_aabb.resolve_collision(&old_aabb, &cell_aabb);
            }
        }
    }

    // Check for and resolve collision with all solid entities except this one
    // TODO: update ECS query to filter to not borrow twice instead of doing it manually here
    for e in entities.values() {
        if e.id != *id {
            if let Some(other_position) = utils::ref_opt_to_opt_ref(e.position.borrow()) {
                if let Some(other_collision_component) =
                    utils::ref_opt_to_opt_ref(e.collision_component.borrow())
                {
                    if other_collision_component.solid {
                        let other_aabb = AABB::from_pos_and_hitbox(
                            *other_position,
                            other_collision_component.hitbox_dimensions,
                        );

                        // TODO: trigger HardCollision script
                        // if new_aabb.is_colliding(&other_aabb) {
                        //     ...
                        // }

                        new_aabb.resolve_collision(&old_aabb, &other_aabb);
                    }
                }
            }
        }
    }

    *position = new_aabb.get_center();
}

pub fn standing_cell(position: &WorldPos) -> CellPos {
    position.to_cellpos()
}

pub fn facing_cell(position: &WorldPos, facing: Direction) -> CellPos {
    let maximum_distance = 0.6;
    let facing_cell_position = match facing {
        Direction::Up => *position + WorldPos::new(0.0, -maximum_distance),
        Direction::Down => *position + WorldPos::new(0.0, maximum_distance),
        Direction::Left => *position + WorldPos::new(-maximum_distance, 0.0),
        Direction::Right => *position + WorldPos::new(maximum_distance, 0.0),
    };
    facing_cell_position.to_cellpos()
}
