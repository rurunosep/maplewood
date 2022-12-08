use crate::tilemap::{self, Cell, CellPos, Point};
use array2d::Array2D;

pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

pub struct PlayerEntity {
    pub position: Point,
    pub direction: Direction,
    pub speed: f64,
    pub hitbox_width: f64,
    pub hitbox_height: f64,
}

pub fn move_player_and_resolve_collisions(
    player: &mut PlayerEntity,
    tilemap: &Array2D<Cell>,
) {
    let mut new_position = player.position
        + match player.direction {
            Direction::Up => Point::new(0.0, -player.speed),
            Direction::Down => Point::new(0.0, player.speed),
            Direction::Left => Point::new(-player.speed, 0.0),
            Direction::Right => Point::new(player.speed, 0.0),
        };

    let new_top = new_position.y - player.hitbox_height / 2.0;
    let new_bot = new_position.y + player.hitbox_height / 2.0;
    let new_left = new_position.x - player.hitbox_width / 2.0;
    let new_right = new_position.x + player.hitbox_width / 2.0;

    // TODO: collision handling maybe should be done by the map?
    // It really just needs access to the player's position, size, direction, etc
    // The map knows more about the objects of the world than the player
    // Eventually this might be component based? Might.

    let points_to_check_for_cell_collision = match player.direction {
        Direction::Up => [Point::new(new_left, new_top), Point::new(new_right, new_top)],
        Direction::Down => {
            [Point::new(new_left, new_bot), Point::new(new_right, new_bot)]
        }
        Direction::Left => [Point::new(new_left, new_top), Point::new(new_left, new_bot)],
        Direction::Right => {
            [Point::new(new_right, new_top), Point::new(new_right, new_bot)]
        }
    };

    for point in points_to_check_for_cell_collision {
        match tilemap::get_cell_at_cellpos(&tilemap, point.to_cellpos()) {
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
                            new_position.y = cell_bot + player.hitbox_height / 2.0
                        }
                        Direction::Down => {
                            new_position.y = cell_top - player.hitbox_height / 2.0
                        }
                        Direction::Left => {
                            new_position.x = cell_right + player.hitbox_width / 2.0
                        }
                        Direction::Right => {
                            new_position.x = cell_left - player.hitbox_width / 2.0
                        }
                    }
                }
            }
            _ => {}
        }
    }

    player.position = new_position;
}

pub fn standing_cell(player: &PlayerEntity) -> CellPos {
    player.position.to_cellpos()
}

pub fn facing_cell(player: &PlayerEntity) -> CellPos {
    let standing_cell = standing_cell(player);
    match player.direction {
        Direction::Up => CellPos::new(standing_cell.x, standing_cell.y - 1),
        Direction::Down => CellPos::new(standing_cell.x, standing_cell.y + 1),
        Direction::Left => CellPos::new(standing_cell.x - 1, standing_cell.y),
        Direction::Right => CellPos::new(standing_cell.x + 1, standing_cell.y),
    }
}
