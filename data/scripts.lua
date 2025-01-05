---@diagnostic disable: unreachable-code

--# start

message("You're sleepy.")
message("But you need a plushy.")
message("Legend tells that the kid in the classroom has a plushy.")

--# school_kid

local stages = {
  [1] = function()
    message("You need a plushy?\n" ..
      "I have one.\n" ..
      "But I need your help.")
    message("I skipped class yesterday, and I need you to write my\n" ..
      "name in the attendance book.")
    message("Get the teacher's pen from the toilet.")

    set_story_var("school_kid::stage", 2)
  end,

  [2] = function()
    message("I'll tell you where the plushy is when you put my name\n" ..
      "in the book.")
  end,

  [3] = function()
    message("Thanks a lot!")
    message("The plushy is in the gym.")

    set_story_var("school_kid::stage", 4)
  end,

  [4] = function()
    message("The plushy is in the gym.")
  end
}

stages[get_story_var("school_kid::stage")]()

--# janitor

local stages = {
  [1] = function()
    message("So tired.")
  end,

  [2] = function()
    message("You need the key?")
    message("I need energy to workout.")
    message("Get me a Super Sugar Bun from the bakery and I'll\n" ..
      "give you the key.")

    set_story_var("janitor::stage", 3)
    set_story_var("bakery_girl::stage", 2)
  end,

  [3] = function()
    message("I need that bun.")
  end,

  [4] = function()
    message("Thanks a bunch! Now I can run.")
    message("Here's the key.")

    set_story_var("janitor::stage", 5)
    set_story_var("bathroom::door::have_key", 1)
  end,

  [5] = function()
    message("Now I can run.")
  end,

  -- is running, but can't crash yet
  [6] = function() end,
  -- is running and may crash
  [7] = function() end,
  -- has crashed
  [8] = function() end
}

stages[get_story_var("janitor::stage")]()

--# bakery_girl

local stages = {
  [1] = function()
    message("I sell buns.")
  end,

  [2] = function()
    message("\"I'll get you your bun.\"")

    set_cutscene_border()
    lock_player_input()
    remove_camera_target()

    walk("CAMERA", "up", 4, 0.05)
    walk_wait("bakery_girl", "up", 0.75, 0.08)
    walk_wait("bakery_girl", "left", 8, 0.08)
    walk_wait("bakery_girl", "up", 4.5, 0.08)
    walk_wait("bakery_girl", "right", 6.5, 0.08)
    walk_wait("bakery_girl", "up", 0, 0.08)
    wait(1)
    walk_wait("bakery_girl", "left", 6.5, 0.08)
    walk_wait("bakery_girl", "down", 4.5, 0.08)
    walk_wait("bakery_girl", "right", 8, 0.08)
    walk_wait("bakery_girl", "down", 0.4, 0.08)
    wait(0.5)
    message("\"Here's your bun.\"")
    wait(1)
    set_entity_visible("bakery::fire", true)
    play_sfx("flame")
    wait(1)
    walk("CAMERA", "down", 4, 0.05)
    wait(2)
    message("\"Take care!\"")

    set_camera_target("player")
    unlock_player_input()
    remove_cutscene_border()

    set_story_var("bakery_girl::stage", 3)
    set_story_var("janitor::stage", 4)
  end,

  [3] = function()
    message("Have a nice day.")
  end,

  -- may start panicking
  [4] = function() end,
  -- has started panicking
  [5] = function() end
}

stages[get_story_var("bakery_girl::stage")]()

--# bakery_girl::panic

while true do
  walk_wait("bakery_girl", "left", 2, 0.12)
  walk_wait("bakery_girl", "right", 2, 0.12)
end

--# bathroom::door

if get_story_var("bathroom::door::open") == 0 then
  if get_story_var("bathroom::door::have_key") == 0 then
    if get_story_var("school_kid::stage") == 2 then
      message("Get the key from the janitor in the gym.")

      if get_story_var("janitor::stage") == 1 then
        set_story_var("janitor::stage", 2)
      end
    end
  else
    switch_dual_state_animation("bathroom::door", 2)
    set_entity_solid("bathroom::door::blocker", false)

    set_story_var("bathroom::door::open", 1)
  end
end

--# bathroom::toilet

if get_story_var("main::pen_found") == 0 then
  message("You find the pen.")

  set_story_var("main::pen_found", 1)
  set_story_var("school_kid::stage", 3)

  set_entity_world_pos("bakery_girl", "hallway", 7.5, 4.5)
  set_entity_solid("bakery_girl", false)
  set_story_var("bakery_girl::stage", 4)
  set_entity_visible("hallway::bakery_fire", true)
  set_entity_visible("hallway::bakery_firefighter", true)
  set_entity_visible("hallway::bakery_water_jet", true)
  set_entity_solid("hallway::bakery_entrance_blocker", true)

  set_entity_map_pos("janitor", 7, 12)
  play_named_animation("janitor", "sprinting", true)
  emit_entity_sfx("janitor", "running", true)
  play_object_animation("gym::treadmill_right", true)
  set_story_var("janitor::stage", 6)
end

--# bathroom::exit

if get_story_var("main::pen_found") == 1 and get_story_var("bathroom::flooded") == 0 then
  set_story_var("bathroom::flooded", 1)

  set_cutscene_border()
  lock_player_input()
  remove_camera_target()

  set_entity_world_pos("player", "hallway", 3.5, 3.5)

  wait(1)
  walk_to_wait("CAMERA", "up", 6.01, 0.05)
  wait(1)
  switch_dual_state_animation("bathroom::sink_1", 2)
  play_sfx("faucet")
  wait(1)
  switch_dual_state_animation("bathroom::sink_2", 2)
  play_sfx("faucet")
  wait(1)
  switch_dual_state_animation("bathroom::bathtub", 2)
  play_sfx("faucet")
  wait(4)

  set_camera_target("player")
  unlock_player_input()
  remove_cutscene_border()
else
  set_entity_world_pos("player", "hallway", 3.5, 3.5)
end

--# overworld::shopping_cart

if get_story_var("bakery_girl::stage") == 3 then
  message("Got a quarter.")

  set_story_var("bakery_girl::stage", 4)
end

--# gym::punching_bag

if get_story_var("school_kid::stage") == 4 and get_story_var("main::plushy_found") == 0 then
  message("You found the plushy!")
  message("Now go outside and find somewhere to sleep.")

  set_story_var("main::plushy_found", 1)

  set_entity_visible("hallway::bathroom_fire", true)
  set_entity_visible("hallway::small_fire_1", true)
  set_entity_visible("hallway::small_fire_2", true)
  set_entity_visible("hallway::bathroom_firefighter", true)
  set_entity_visible("hallway::bathroom_water_jet", true)
  set_entity_solid("hallway::bathroom_entrance_blocker", true)

  set_story_var("janitor::stage", 7)
end

--# overworld::garbage_bin

if get_story_var("main::plushy_found") == 1 then
  message("This is a great place to sleep.")
  message("Goodnight!")

  close_game()
end

--# hallway::janitor_crash_trigger

stop_object_animation("janitor")
stop_entity_sfx("janitor")
set_forced_sprite("janitor", "janitor_down", 0, 0, 32, 16, 8, 8)
set_entity_map_pos("janitor", 7, 14.8)
set_entity_solid("janitor", false)

set_entity_visible("hallway::wall_crack", true)
play_sfx("rock_smash")

--#
