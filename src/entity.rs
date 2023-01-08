use crate::script::Script;
use crate::world::{self, Cell, CellPos, Point, WorldPos};
use array2d::Array2D;
use sdl2::rect::Rect;
use std::cell::RefCell;

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
    // Entities might want to keep track of an ID or name or something
    // But for now, storing and indexing them with a hardcoded name works just fine
    pub position: RefCell<Option<WorldPos>>,
    pub sprite_component: RefCell<Option<SpriteComponent>>,
    // Both the sprite facing direction and the walking direction
    pub facing: RefCell<Option<Direction>>,
    pub walking_component: RefCell<Option<WalkingComponent>>,
    // TODO: handle collision script triggering with this
    pub collision_component: RefCell<Option<CollisionComponent>>,
    pub scripts: RefCell<Option<Vec<Script>>>,
}

#[derive(Clone, Debug)]
pub struct SpriteComponent {
    // The region of the full spritesheet with this entity's sprites
    pub spriteset_rect: Rect,
    pub sprite_offset: Point<i32>,
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
    pub enabled: bool,
}

pub fn walk_and_resolve_tile_collisions(
    position: &mut WorldPos,
    walking_component: &WalkingComponent,
    collision_component: &CollisionComponent,
    tilemap: &Array2D<Cell>,
) {
    let mut new_position = *position
        + match walking_component.direction {
            Direction::Up => WorldPos::new(0.0, -walking_component.speed),
            Direction::Down => WorldPos::new(0.0, walking_component.speed),
            Direction::Left => WorldPos::new(-walking_component.speed, 0.0),
            Direction::Right => WorldPos::new(walking_component.speed, 0.0),
        };

    let new_top = new_position.y - collision_component.hitbox_dimensions.y / 2.0;
    let new_bot = new_position.y + collision_component.hitbox_dimensions.y / 2.0;
    let new_left = new_position.x - collision_component.hitbox_dimensions.x / 2.0;
    let new_right = new_position.x + collision_component.hitbox_dimensions.x / 2.0;

    if collision_component.enabled {
        let points_to_check_for_cell_collision = match walking_component.direction {
            Direction::Up => {
                [WorldPos::new(new_left, new_top), WorldPos::new(new_right, new_top)]
            }
            Direction::Down => {
                [WorldPos::new(new_left, new_bot), WorldPos::new(new_right, new_bot)]
            }
            Direction::Left => {
                [WorldPos::new(new_left, new_top), WorldPos::new(new_left, new_bot)]
            }
            Direction::Right => {
                [WorldPos::new(new_right, new_top), WorldPos::new(new_right, new_bot)]
            }
        };

        for point in points_to_check_for_cell_collision {
            match world::get_cell_at_cellpos(tilemap, point.to_cellpos()) {
                Some(cell) if !cell.passable => {
                    let cell_top = point.y.floor();
                    let cell_bot = point.y.ceil();
                    let cell_left = point.x.floor();
                    let cell_right = point.x.ceil();
                    if new_top < cell_bot
                        && new_bot > cell_top
                        && new_left < cell_right
                        && new_right > cell_left
                    {
                        match walking_component.direction {
                            Direction::Up => {
                                new_position.y =
                                    cell_bot + collision_component.hitbox_dimensions.y / 2.0
                            }
                            Direction::Down => {
                                new_position.y =
                                    cell_top - collision_component.hitbox_dimensions.y / 2.0
                            }
                            Direction::Left => {
                                new_position.x =
                                    cell_right + collision_component.hitbox_dimensions.x / 2.0
                            }
                            Direction::Right => {
                                new_position.x =
                                    cell_left - collision_component.hitbox_dimensions.x / 2.0
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let map_width = tilemap.num_columns() as f64;
    let map_height = tilemap.num_rows() as f64;
    if new_top < 0.0 {
        new_position.y = 0.0 + collision_component.hitbox_dimensions.y / 2.0;
    }
    if new_bot > map_height {
        new_position.y = map_height - collision_component.hitbox_dimensions.y / 2.0;
    }
    if new_left < 0.0 {
        new_position.x = 0.0 + collision_component.hitbox_dimensions.x / 2.0;
    }
    if new_right > map_width {
        new_position.x = map_width - collision_component.hitbox_dimensions.x / 2.0;
    }

    *position = new_position;
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
