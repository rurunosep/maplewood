use crate::world::{self, Cell, CellPos, Point, WorldPos};
use array2d::Array2D;
use sdl2::rect::Rect;

#[derive(Default, Clone, Copy)]
pub enum Direction {
    Up,
    #[default]
    Down,
    Left,
    Right,
}

// TODO: Maybe a consise way to query for entities with certain components

#[derive(Clone, Default)]
pub struct Entity {
    // Entities might want to keep track of an ID or name or something
    // But for now, storing and indexing them with a hardcoded name works just fine
    pub position: WorldPos,
    pub character_component: Option<CharacterComponent>,
    pub player_component: Option<PlayerComponent>,
    // TODO: Maybe want to combine these into a single ScriptComponent which contains
    // scripts with various triggers
    pub interaction_component: Option<InteractionComponent>,
    pub collision_component: Option<CollisionComponent>,
}

#[derive(Clone)]
pub struct CharacterComponent {
    // The region of the full spritesheet with this entity's sprites
    pub spriteset_rect: Rect,
    pub sprite_offset: Point<i32>,
    // Currently both the sprite facing direction, and the player's moving direction
    pub direction: Direction,
}

#[derive(Clone)]
pub struct PlayerComponent {
    pub speed: f64,
    pub hitbox_dimensions: Point<f64>,
}

// Start a script when player interacts while facing entity's cell
#[derive(Clone)]
pub struct InteractionComponent {
    pub script_source: String,
}

// Start a script when player stands on entity's cell
#[derive(Clone)]
pub struct CollisionComponent {
    pub script_source: String,
}

pub fn move_player_and_resolve_collisions(player: &mut Entity, tilemap: &Array2D<Cell>) {
    let direction = player.character_component.as_ref().unwrap().direction;
    let PlayerComponent { speed, hitbox_dimensions } =
        *player.player_component.as_ref().unwrap();

    let mut new_position = player.position
        + match direction {
            Direction::Up => WorldPos::new(0.0, -speed),
            Direction::Down => WorldPos::new(0.0, speed),
            Direction::Left => WorldPos::new(-speed, 0.0),
            Direction::Right => WorldPos::new(speed, 0.0),
        };

    let new_top = new_position.y - hitbox_dimensions.y / 2.0;
    let new_bot = new_position.y + hitbox_dimensions.y / 2.0;
    let new_left = new_position.x - hitbox_dimensions.x / 2.0;
    let new_right = new_position.x + hitbox_dimensions.x / 2.0;

    let points_to_check_for_cell_collision = match direction {
        Direction::Up => [WorldPos::new(new_left, new_top), WorldPos::new(new_right, new_top)],
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
                    match direction {
                        Direction::Up => new_position.y = cell_bot + hitbox_dimensions.y / 2.0,
                        Direction::Down => {
                            new_position.y = cell_top - hitbox_dimensions.y / 2.0
                        }
                        Direction::Left => {
                            new_position.x = cell_right + hitbox_dimensions.x / 2.0
                        }
                        Direction::Right => {
                            new_position.x = cell_left - hitbox_dimensions.x / 2.0
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let map_width = tilemap.num_columns() as f64;
    let map_height = tilemap.num_rows() as f64;
    if new_top < 0.0 {
        new_position.y = 0.0 + hitbox_dimensions.y / 2.0;
    }
    if new_bot > map_height {
        new_position.y = map_height - hitbox_dimensions.y / 2.0;
    }
    if new_left < 0.0 {
        new_position.x = 0.0 + hitbox_dimensions.x / 2.0;
    }
    if new_right > map_width {
        new_position.x = map_width - hitbox_dimensions.x / 2.0;
    }

    player.position = new_position;
}

pub fn standing_cell(entity: &Entity) -> CellPos {
    entity.position.to_cellpos()
}

pub fn facing_cell(entity: &Entity) -> CellPos {
    // TODO: confirm that entity has character component and return option or result?
    let maximum_distance = 0.6;
    let facing_cell_position = match entity.character_component.as_ref().unwrap().direction {
        Direction::Up => entity.position + WorldPos::new(0.0, -maximum_distance),
        Direction::Down => entity.position + WorldPos::new(0.0, maximum_distance),
        Direction::Left => entity.position + WorldPos::new(-maximum_distance, 0.0),
        Direction::Right => entity.position + WorldPos::new(maximum_distance, 0.0),
    };
    facing_cell_position.to_cellpos()
}
