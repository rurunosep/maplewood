---@diagnostic disable: unreachable-code

---@script start
---@start_condition {start_script::started} == 0
set("start_script::started", 1)

message("Message widows are skinned now.", 0)
message("And they can type out like this.")
message("So far they've needed a click to advance.")
message("But this one advances itself a second after it's finished.", nil, 1)
message(
  "This one needs a click again, but it's really long. You can click to skip the typing effect. It's a whole lot of yap. And then you click again to advance it.")
message("This one types out really fast.", 200)
message("And this one is slow.", 10)

---@script twinkle

message("Twinkle, Twinkle, Little Star")

play_music("twinkle")

sing_word_wait("_player", "Twinkle, ", 0, 1)
sing_word_wait("_player", "twinkle, ", 1, 1)
sing_word_wait("_player", "little ", 2, 1)
sing_word_wait("_player", "star,", 1, 1)
clear_singing("_player")

sing_word_wait("_player", "How I ", 1, 1)
sing_word_wait("_player", "wonder ", 0, 1)
sing_word_wait("_player", "what you ", -1, 1)
sing_word_wait("_player", "are,", -2, 1)
clear_singing("_player")

sing_word_wait("_player", "Up a-", 1, 1)
sing_word_wait("_player", "bove the ", 0, 1)
sing_word_wait("_player", "world so ", -1, 1)
sing_word_wait("_player", "high,", -2, 1)
clear_singing("_player")

sing_word_wait("_player", "Like a ", 1, 1)
sing_word_wait("_player", "diamond ", 0, 1)
sing_word_wait("_player", "in the ", -1, 1)
sing_word_wait("_player", "sky,", -2, 1)
clear_singing("_player")

sing_word_wait("_player", "Twinkle, ", 0, 1)
sing_word_wait("_player", "twinkle, ", 1, 1)
sing_word_wait("_player", "little ", 2, 1)
sing_word_wait("_player", "star,", 1, 1)
clear_singing("_player")

sing_word_wait("_player", "How I ", 1, 1)
sing_word_wait("_player", "wonder ", 0, 1)
sing_word_wait("_player", "what you ", -1, 1)
sing_word_wait("_player", "are.", -2, 1)
clear_singing("_player")

wait(1)

set_facing("_player", "left")
wait(0.1)
set_facing("_player", "up")
wait(0.1)
set_facing("_player", "right")
wait(0.1)
set_facing("_player", "down")

---@script cameras

set_camera_visible("_camera", false)
set_camera_visible("corner_camera", false)

set_camera_overlay_color("top_left_camera", { 0, 0, 0, 1 })
set_camera_visible("top_left_camera", true)
set_camera_zoom("top_left_camera", 6)
set_camera_target("top_left_camera", "_player")

set_camera_overlay_color("top_right_camera", { 0, 0, 0, 1 })
set_camera_visible("top_right_camera", true)
set_camera_zoom("top_right_camera", 6)
set_camera_target("top_right_camera", "bakery_girl")

set_camera_overlay_color("bottom_left_camera", { 0, 0, 0, 1 })
set_camera_visible("bottom_left_camera", true)
set_camera_zoom("bottom_left_camera", 6)
set_camera_target("bottom_left_camera", "school_kid")

set_camera_overlay_color("bottom_right_camera", { 0, 0, 0, 1 })
set_camera_visible("bottom_right_camera", true)
set_camera_zoom("bottom_right_camera", 6)
set_camera_target("bottom_right_camera", "janitor")

set_camera_overlay_color("top_left_camera", { 0, 0, 0, 0 }, 1)
wait(1)
set_camera_overlay_color("top_right_camera", { 0, 0, 0, 0 }, 1)
wait(1)
set_camera_overlay_color("bottom_left_camera", { 0, 0, 0, 0 }, 1)
wait(1)
set_camera_overlay_color("bottom_right_camera", { 0, 0, 0, 0 }, 1)
wait(1)

---@script star_move

while true do
  walk_to_wait("_player", 0, 4)
  walk_to_wait("_player", 1, 1.5)
  walk_to_wait("_player", 2, 4, 0.05)
  walk_to_wait("_player", -0.5, 2.5, 0.25)
  walk_to_wait("_player", 2.5, 2.5, 0.25)
end

---@script face

while true do
  x, y = get_entity_map_pos("_player")
  set_facing_towards_point("bakery_girl", x, y)
  yield()
end

---@script school_kid

local stages = {
  [1] = function()
    message("\"You need a plushy?\n" ..
      "I have one hidden somewhere.\n" ..
      "But I need your help.\"")
    message("\"I skipped class yesterday, and I need to write my\n" ..
      "name in the attendance book.\"")
    message("\"Bring me the teacher's special pen from the broken\n" ..
      "toilet in the bathroom.\"")

    set("school_kid::stage", 2)
  end,

  [2] = function()
    message("\"I'll tell you where the plushy is when you get me the\n" ..
      "pen.\"")
  end,

  [3] = function()
    message("\"Thanks a lot!\"")
    message("\"The plushy is behind a punching bag in the gym.\"")

    set("school_kid::stage", 4)
  end,

  [4] = function()
    message("\"The plushy is behind a punching bag in the gym.\"")
  end
}

stages[get("school_kid::stage")]()

---@script janitor

local stages = {
  [1] = function()
    message("\"I'm so tired today...\"")
  end,

  [2] = function()
    message("\"You need the key to the bathroom?\"")
    message("\"I need some carbs for my workout.\"")
    message("\"Get me a Super Sugar Bun from the bakery and I'll\n" ..
      "give you the key.\"")

    set("janitor::stage", 3)
    set("bakery_girl::stage", 2)
  end,

  [3] = function()
    message("\"I need that bun.\"")
  end,

  [4] = function()
    message("\"Thanks a bunch! Now I can run.\"")
    message("\"Here's the key.\"")

    set("janitor::stage", 5)
    set("bathroom::door::have_key", 1)
  end,

  [5] = function()
    message("\"Now I can run.\"")
  end,

  -- is running, but can't crash yet
  [6] = function() end,
  -- is running and may crash
  [7] = function() end,
  -- has crashed
  [8] = function() end
}

stages[get("janitor::stage")]()

---@script bakery_girl

local stages = {
  [1] = function()
    message("\"I sell buns!\"")
  end,

  [2] = function()
    message("\"You need a Super Sugar Bun? Coming right up!\"")

    lock_player_input()
    set_camera_target("_camera", nil)

    walk("_camera", "up", 4, 3)

    set_walk_speed("bakery_girl", 5)
    walk_wait("bakery_girl", "up", 0.75)
    walk_wait("bakery_girl", "left", 8)
    walk_wait("bakery_girl", "up", 4.5)
    walk_wait("bakery_girl", "right", 6.5)
    set_facing("bakery_girl", "up")
    wait(1)
    walk_wait("bakery_girl", "left", 6.5)
    walk_wait("bakery_girl", "down", 4.5)
    walk_wait("bakery_girl", "right", 8)
    walk_wait("bakery_girl", "down", 0.4)
    wait(0.5)
    message("\"Here's your bun!\"")
    wait(1)
    set_entity_visible("bakery::fire", true)
    play_sfx("flame")
    wait(1)
    walk("_camera", "down", 4)
    wait(2)
    message("\"Take care!\"")

    set_camera_target("_camera", "_player")
    unlock_player_input()

    set("bakery_girl::stage", 3)
    set("janitor::stage", 4)
  end,

  [3] = function()
    message("\"Have a nice day.\"")
  end,

  -- may start panicking
  [4] = function() end,
  -- has started panicking
  [5] = function() end
}

stages[get("bakery_girl::stage")]()

---@script bakery_girl::panic
---@start_condition {bakery_girl::stage} == 4
set("bakery_girl::stage", 5)

set_walk_speed("bakery_girl", 7)

while true do
  walk_wait("bakery_girl", "left", 2)
  walk_wait("bakery_girl", "right", 2)
end

---@script bathroom::door

if get("bathroom::door::open") == 0 then
  if get("bathroom::door::have_key") == 0 then
    if get("school_kid::stage") == 2 then
      message("There's a note on the door:")
      message("\"Closed for repairs. If you need to get in, find me\n" ..
        "in the gym.\" - Janitor")

      if get("janitor::stage") == 1 then
        set("janitor::stage", 2)
      end
    end
  else
    switch_dual_state_animation("bathroom::door", 2)
    set_entity_solid("bathroom::door::blocker", false)

    set("bathroom::door::open", 1)
  end
end

---@script bathroom::toilet

if get("main::pen_found") == 0 then
  message("You found the pen.")

  set("main::pen_found", 1)
  set("school_kid::stage", 3)

  set_entity_world_pos("bakery_girl", "hallway", 7.5, 4.5)
  set_entity_solid("bakery_girl", false)
  set("bakery_girl::stage", 4)
  set_entity_visible("hallway::bakery_fire", true)
  set_entity_visible("hallway::bakery_firefighter", true)
  set_entity_visible("hallway::bakery_water_jet", true)
  set_entity_solid("hallway::bakery_entrance_blocker", true)

  set_entity_map_pos("janitor", 7, 12)
  play_named_animation("janitor", "sprinting", true)
  emit_entity_sfx("janitor", "running", true)
  play_object_animation("gym::treadmill_right", true)
  set("janitor::stage", 6)
end

---@script bathroom::exit

if get("main::pen_found") == 1 and get("bathroom::flooded") == 0 then
  set("bathroom::flooded", 1)

  lock_player_input()
  set_camera_target("_camera", nil)
  set_camera_clamp("_camera", false)

  set_entity_world_pos("_player", "hallway", 3.5, 3.5)

  wait(1)
  walk_wait("_camera", "up", 6.01, 3)
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

  set_camera_clamp("_camera", true)
  set_camera_target("_camera", "_player")
  unlock_player_input()
else
  set_entity_world_pos("_player", "hallway", 3.5, 3.5)
end

---@script gym::punching_bag

if get("school_kid::stage") == 4 and get("main::plushy_found") == 0 then
  message("You found the plushy!")
  message("Now go outside and find somewhere cozy to sleep.")

  set("main::plushy_found", 1)

  set_entity_visible("hallway::bathroom_fire", true)
  set_entity_visible("hallway::small_fire_1", true)
  set_entity_visible("hallway::small_fire_2", true)
  set_entity_visible("hallway::bathroom_firefighter", true)
  set_entity_visible("hallway::bathroom_water_jet", true)
  set_entity_solid("hallway::bathroom_entrance_blocker", true)

  set("janitor::stage", 7)
end

---@script overworld::garbage_bin

if get("main::plushy_found") == 1 then
  message("This place is perfect to sleep!")
  message("Goodnight!")

  close_game()
end

---@script hallway::janitor_crash_trigger

if get("janitor::stage") ~= 7 then
  return
end

set("janitor::stage", 8)

stop_object_animation("janitor")
stop_entity_sfx("janitor")
set_forced_sprite("janitor", "janitor_down", 0, 0, 32, 16, 8, 8)
set_entity_map_pos("janitor", 7, 14.8)
set_entity_solid("janitor", false)

set_entity_visible("hallway::wall_crack", true)
play_sfx("rock_smash")

---@script bakery_girl_panic_setup

set_entity_world_pos("bakery_girl", "hallway", 7.5, 4.5)
set_entity_solid("bakery_girl", false)
