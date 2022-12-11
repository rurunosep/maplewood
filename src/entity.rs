use crate::world::{self, Cell, CellPos, Point, WorldPos};
use array2d::Array2D;

#[derive(Default)]
pub enum Direction {
    Up,
    #[default]
    Down,
    Left,
    Right,
}

#[derive(Default)]
pub struct PlayerEntity {
    pub position: WorldPos,
    pub direction: Direction,
    pub speed: f64,
    pub hitbox_dimensions: Point<f64>,
    pub sprite_offset: Point<i32>,
}

pub fn move_player_and_resolve_collisions(player: &mut PlayerEntity, tilemap: &Array2D<Cell>) {
    let mut new_position = player.position
        + match player.direction {
            Direction::Up => WorldPos::new(0.0, -player.speed),
            Direction::Down => WorldPos::new(0.0, player.speed),
            Direction::Left => WorldPos::new(-player.speed, 0.0),
            Direction::Right => WorldPos::new(player.speed, 0.0),
        };

    let new_top = new_position.y - player.hitbox_dimensions.y / 2.0;
    let new_bot = new_position.y + player.hitbox_dimensions.y / 2.0;
    let new_left = new_position.x - player.hitbox_dimensions.x / 2.0;
    let new_right = new_position.x + player.hitbox_dimensions.x / 2.0;

    let points_to_check_for_cell_collision = match player.direction {
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
        match world::get_cell_at_cellpos(&tilemap, point.to_cellpos()) {
            Some(cell) if cell.passable == false => {
                let cell_top = point.y.floor();
                let cell_bot = point.y.ceil();
                let cell_left = point.x.floor();
                let cell_right = point.x.ceil();
                if new_top < cell_bot
                    && new_bot > cell_top
                    && new_left < cell_right
                    && new_right > cell_left
                {
                    match player.direction {
                        Direction::Up => {
                            new_position.y = cell_bot + player.hitbox_dimensions.y / 2.0
                        }
                        Direction::Down => {
                            new_position.y = cell_top - player.hitbox_dimensions.y / 2.0
                        }
                        Direction::Left => {
                            new_position.x = cell_right + player.hitbox_dimensions.x / 2.0
                        }
                        Direction::Right => {
                            new_position.x = cell_left - player.hitbox_dimensions.x / 2.0
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
        new_position.y = 0.0 + player.hitbox_dimensions.y / 2.0;
    }
    if new_bot > map_height {
        new_position.y = map_height - player.hitbox_dimensions.y / 2.0;
    }
    if new_left < 0.0 {
        new_position.x = 0.0 + player.hitbox_dimensions.x / 2.0;
    }
    if new_right > map_width {
        new_position.x = map_width - player.hitbox_dimensions.x / 2.0;
    }

    player.position = new_position;
}

pub fn standing_cell(player: &PlayerEntity) -> CellPos {
    player.position.to_cellpos()
}

pub fn facing_cell(player: &PlayerEntity) -> CellPos {
    let maximum_distance = 0.6;
    let facing_cell_position = match player.direction {
        Direction::Up => player.position + WorldPos::new(0.0, -maximum_distance),
        Direction::Down => player.position + WorldPos::new(0.0, maximum_distance),
        Direction::Left => player.position + WorldPos::new(-maximum_distance, 0.0),
        Direction::Right => player.position + WorldPos::new(maximum_distance, 0.0),
    };
    facing_cell_position.to_cellpos()
}
