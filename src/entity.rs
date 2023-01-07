use crate::script::{Script, ScriptCondition, ScriptTrigger};
use crate::world::{self, Cell, CellPos, Point, WorldPos};
use array2d::Array2D;
use sdl2::rect::Rect;
use std::cell::RefCell;
use std::collections::HashMap;

#[macro_export]
macro_rules! ecs_query {
    ($entities:ident[$name:expr], $($a:ident $($b:ident)?),*) => {
        $entities.get($name).map(|e| Some((
            $( ecs_query!(impl e $a $($b)?), )*
        ))).flatten()
    };

    ($entities:ident, $($a:ident $($b:ident)?),*) => {
        $entities.values().filter_map(|e| Some((
            $( ecs_query!(impl e $a $($b)?), )*
        )))
    };

    (impl $e:ident mut $component:ident) => {
        crate::utils::refmut_opt_to_opt_refmut($e.$component.borrow_mut())?
    };

    (impl $e:ident $component:ident) => {
        crate::utils::ref_opt_to_opt_ref($e.$component.borrow())?
    };
}

#[derive(Default, Clone, Copy, Debug)]
pub enum Direction {
    Up,
    #[default]
    Down,
    Left,
    Right,
}

#[derive(Clone, Default)]
pub struct Entity {
    // Entities might want to keep track of an ID or name or something
    // But for now, storing and indexing them with a hardcoded name works just fine
    pub position: RefCell<Option<WorldPos>>,
    pub character_component: RefCell<Option<CharacterComponent>>,
    pub player_component: RefCell<Option<PlayerComponent>>,
    pub script_component: RefCell<Option<ScriptComponent>>,
}

#[derive(Clone, Debug)]
// TODO: split this
pub struct CharacterComponent {
    // The region of the full spritesheet with this entity's sprites
    pub spriteset_rect: Rect,
    pub sprite_offset: Point<i32>,
    // Currently both the sprite facing direction, and the player's moving direction
    pub direction: Direction,
}

#[derive(Clone, Debug)]
pub struct PlayerComponent {
    pub speed: f64,
    pub hitbox_dimensions: Point<f64>,
}

#[derive(Clone, Debug)]
pub struct ScriptComponent {
    // Maybe eventually this is references to scripts stored somewhere else
    pub scripts: Vec<Script>,
}

impl ScriptComponent {
    pub fn filter_scripts_by_trigger_and_condition(
        &mut self,
        filter_trigger: ScriptTrigger,
        story_vars: &HashMap<String, i32>,
    ) -> Vec<&mut Script> {
        self.scripts
            .iter_mut()
            .filter(|script| script.trigger == filter_trigger)
            .filter(|script| {
                script.start_condition.is_none() || {
                    let ScriptCondition { story_var, value } =
                        script.start_condition.as_ref().unwrap();
                    *story_vars.get(story_var).unwrap() == *value
                }
            })
            .collect()
    }
}

pub fn move_player_and_resolve_collisions(
    entities: &HashMap<String, Entity>,
    tilemap: &Array2D<Cell>,
) {
    let (mut position, character_component, player_component) =
        ecs_query!(entities["player"], mut position, character_component, player_component)
            .unwrap();

    let mut new_position = *position
        + match character_component.direction {
            Direction::Up => WorldPos::new(0.0, -player_component.speed),
            Direction::Down => WorldPos::new(0.0, player_component.speed),
            Direction::Left => WorldPos::new(-player_component.speed, 0.0),
            Direction::Right => WorldPos::new(player_component.speed, 0.0),
        };

    let new_top = new_position.y - player_component.hitbox_dimensions.y / 2.0;
    let new_bot = new_position.y + player_component.hitbox_dimensions.y / 2.0;
    let new_left = new_position.x - player_component.hitbox_dimensions.x / 2.0;
    let new_right = new_position.x + player_component.hitbox_dimensions.x / 2.0;

    let points_to_check_for_cell_collision = match character_component.direction {
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
                    match character_component.direction {
                        Direction::Up => {
                            new_position.y =
                                cell_bot + player_component.hitbox_dimensions.y / 2.0
                        }
                        Direction::Down => {
                            new_position.y =
                                cell_top - player_component.hitbox_dimensions.y / 2.0
                        }
                        Direction::Left => {
                            new_position.x =
                                cell_right + player_component.hitbox_dimensions.x / 2.0
                        }
                        Direction::Right => {
                            new_position.x =
                                cell_left - player_component.hitbox_dimensions.x / 2.0
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
        new_position.y = 0.0 + player_component.hitbox_dimensions.y / 2.0;
    }
    if new_bot > map_height {
        new_position.y = map_height - player_component.hitbox_dimensions.y / 2.0;
    }
    if new_left < 0.0 {
        new_position.x = 0.0 + player_component.hitbox_dimensions.x / 2.0;
    }
    if new_right > map_width {
        new_position.x = map_width - player_component.hitbox_dimensions.x / 2.0;
    }

    *position = new_position;
}

pub fn standing_cell(position: &WorldPos) -> CellPos {
    position.to_cellpos()
}

pub fn facing_cell(position: &WorldPos, character_component: &CharacterComponent) -> CellPos {
    let maximum_distance = 0.6;
    let facing_cell_position = match character_component.direction {
        Direction::Up => *position + WorldPos::new(0.0, -maximum_distance),
        Direction::Down => *position + WorldPos::new(0.0, maximum_distance),
        Direction::Left => *position + WorldPos::new(-maximum_distance, 0.0),
        Direction::Right => *position + WorldPos::new(maximum_distance, 0.0),
    };
    facing_cell_position.to_cellpos()
}
