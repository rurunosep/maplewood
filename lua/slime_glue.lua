--# start

set("start_script_started", 1)

set_map_overlay_color(0, 0, 0, 255, 0)
lock_player_input()
set_entity_solid("player", false)
set_forced_sprite("player", "dead", 32, 16, 16, 16)
set_cutscene_border()

message("v0.2.0: Slime Glue \n"
  .. " \n"
  .. "Arrow keys to move. ENTER or SPACE to interact.\n"
  .. "ESC to quit.")

message("Previously, in v0.1.0...")
message("You broke into the house, got the plushy, and \n"
  .. "went to sleep.")

set_map_overlay_color(0, 0, 0, 0, 3)
wait(4)

message("\"... ... ...\"")
anim_quiver("man", 0.5)
message("\"Wake up!\"")

remove_forced_sprite("player")
anim_jump("player")
play_sfx("jump")
walk("player", "down", 1, 0.15)
wait(1)

message("\"You really made a mess here... Smashed my \n"
  .. "garden pot, broke into my house, stole my favorite \n"
  .. "plushy, slept in my bed...\"")
message("\"You're going to have to take responsibility...\"")

anim_quiver("player", 0.8)
play_sfx("quiver")
wait(1)

message("\"Don't worry. If you just clean up your mess, I'll \n"
  .. "give you a nice reward.\"")
message("\"I'm sure you must be pretty hungry after messing \n"
  .. "up my house and napping in my bed...\"")
message("\"Fortunately, I just got back with some delicious \n"
  .. "spaghet.\"")

anim_jump("player")
play_sfx("jump")
wait(0.3)
anim_jump("player")
play_sfx("jump")
wait(0.3)

walk("man", "left", 4, 0.06)
wait(0.7)
walk("player", "left", 0, 1)
wait_until_not_walking("man")
wait(0.5)

walk("man", "right", 0, 1)
wait(0.5)

message("\"Anyway, first of all, put my plushy back in the \n"
  .. "chest where it belongs.\"")

unlock_player_input()
set_entity_solid("player", true)
remove_cutscene_border()

set("look_at_player", 1)

wait_storyvar("put_away_plushy", 1)

lock_player_input()
walk_to("player", "down", 6.5, 0.06)
wait_until_not_walking("player")
walk_to("player", "left", 8.5, 0.06)
wait_until_not_walking("player")
walk_to("player", "right", 8.5, 0.06)
wait_until_not_walking("player")
walk_to("player", "up", 6.5, 0.06)
wait_until_not_walking("player")
unlock_player_input()

set_cell_tile(8, 5, 2, 36)
play_sfx("door_close")
set("look_at_player", 0)
set_cutscene_border()
lock_player_input()
wait(0.5)

message("\"Good. Now come follow me outside.\"")
set_cell_tile(8, 8, 2, -1)
set_cell_passable(8, 8, true)
play_sfx("door_open")
walk_to("man", "down", 10.5, 0.06)
wait(0.7)
walk("player", "down", 4, 0.06)
wait_until_not_walking("man")
walk("man", "right", 4, 0.06)
wait_until_not_walking("player")
walk("player", "right", 0, 1)
wait_until_not_walking("man")
walk("man", "up", 0, 1)
wait(0.5)
message("\"Was this even necessary? You could have just \n"
  .. "reached in and grabbed the key...\"")
wait(0.5)
walk("man", "left", 0, 1)
wait(0.5)
message("\"My wife built this pot for our garden before she \n"
  .. "passed. You're gonna have to fix this somehow.\"")

add_position_component("slime", 5.5, 11.5)
anim_jump("slime")
play_sfx("jump")
walk("slime", "up", 1, 0.15)
wait(0.8)

anim_jump("man")
play_sfx("jump")
wait(0.8)

walk("player", "left", 0, 1)
anim_jump("player")
play_sfx("jump")
wait(1)

message("\"It's a slime!\"")
walk("slime", "right", 0, 1)
wait(1)
message("\"Dead slimes make great glue!\"")
anim_jump("slime")
play_sfx("jump")
wait(1)
message("\"Catch it and kill it to fix the pot!\"")
anim_jump("slime")
play_sfx("jump")
wait(1)

play_music("benny")
set("slime_loop", 1)
remove_cutscene_border()
unlock_player_input()
set("can_touch_slime", 1)

wait_storyvar("times_touched_slime", 1)

lock_player_input()
set_forced_sprite("player", "dead", 32, 0, 16, 16)
play_sfx("slip")
wait(1)
remove_forced_sprite("player")
unlock_player_input()
set("can_touch_slime", 1)

wait_storyvar("times_touched_slime", 2)

stop_music(0)
play_sfx("squish")
set("slime_loop", 0)
walk("slime", "down", 0, 1)
set_forced_sprite("slime", "dead", 16, 32, 16, 16)
set_cutscene_border()
lock_player_input()
wait(0.5)
message("\"You got it!\"")
anim_jump("player")
play_sfx("jump")
wait(0.3)
anim_jump("player")
play_sfx("jump")
wait(0.3)
wait(0.5)
message("\"Now bring it over here and fix the pot!\"")
walk_wait("man", "right", 1, 0.06)
walk("man", "left", 0, 1)
remove_position_component("slime")
remove_cutscene_border()
unlock_player_input()

wait_storyvar("fixed_pot", 1)

set_cutscene_border()
lock_player_input()
play_sfx("squish")
wait(0.8)
play_sfx("squish")
wait(0.8)
play_sfx("squish")
wait(0.8)
set_cell_tile(12, 9, 2, 27)
wait(1)

message("\"That slime glue works wonders! It looks just like \n"
  .. "it did before.\"")
walk("player", "right", 0, 1)
message("\"I think you've cleaned up pretty well for now.\"")
message("\"I'm feeling a bit hungry now. Come inside and \n"
  .. "wait for me to get the spaghet.\"")
anim_jump("player")
play_sfx("jump")

walk_wait("man", "down", 1, 0.06)
walk("man", "left", 5, 0.06)
wait(0.5)
walk("player", "down", 0, 1)
wait_until_not_walking("man")
walk("player", "left", 0, 1)
walk_wait("man", "up", 5, 0.06)
set("door_may_close", 1)
remove_cutscene_border()
unlock_player_input()
walk_wait("man", "left", 2, 0.06)
walk_wait("man", "up", 1, 0.06)
remove_position_component("man")

wait_storyvar("door_may_close", 0)

wait(2)

add_position_component("man", 6.5, 5.5)
walk("man", "down", 0, 1)
set_cutscene_border()
lock_player_input()
message("\"Spaghetti time!\"")
anim_jump("player")
play_sfx("jump")
wait(0.3)
anim_jump("player")
play_sfx("jump")
wait(0.3)
anim_jump("player")
play_sfx("jump")
wait(0.3)
wait(0.5)

play_music("spaghetti_time")
wait(1)
set_map_overlay_color(0, 0, 0, 255, 5)
wait(5)
wait(1)
show_card("spaghetti_time")
wait(6)
stop_music(7)
wait(8)
close_game()

--# slime_collision

local times = get("times_touched_slime")
set("times_touched_slime", times + 1)
set("can_touch_slime", 0)

--# chest

set("put_away_plushy", 1)

--# pot

set("fixed_pot", 1)

--# inside_door

set_cell_tile(8, 8, 2, 48)
set_cell_passable(8, 8, false)
play_sfx("door_close")
set("door_may_close", 0)

--# look_at_player

set("look_at_player", 2)

while (true) do
  local player_x, player_y = get_entity_position("player")
  local man_x, man_y = get_entity_position("man")
  local man_to_player_x = player_x - man_x
  local man_to_player_y = player_y - man_y

  if (math.abs(man_to_player_x) > math.abs(man_to_player_y)) then
    if (man_to_player_x < 0) then
      walk("man", "left", 0, 1)
    else
      walk("man", "right", 0, 1)
    end
  else
    if (man_to_player_y < 0) then
      walk("man", "up", 0, 1)
    else
      walk("man", "down", 0, 1)
    end
  end

  coroutine.yield()
end

--# slime_loop

set("slime_loop", 2)

while (true) do
  walk_wait("slime", "left", 2, 0.2)
  walk_wait("slime", "down", 2, 0.2)
  walk_wait("slime", "right", 1, 0.2)
  walk_wait("slime", "down", 3, 0.2)
  walk_wait("slime", "right", 3, 0.2)
  walk_wait("slime", "down", 2, 0.2)
  walk_wait("slime", "left", 1, 0.2)
  walk_wait("slime", "down", 2, 0.2)
  walk_wait("slime", "right", 3, 0.2)
  walk_wait("slime", "up", 1, 0.2)
  walk_wait("slime", "right", 2, 0.2)
  walk_wait("slime", "up", 3, 0.2)
  walk_wait("slime", "left", 1, 0.2)
  walk_wait("slime", "up", 2, 0.2)
  walk_wait("slime", "left", 4, 0.2)
  walk_wait("slime", "down", 3, 0.2)
  walk_wait("slime", "right", 3, 0.2)
  walk_wait("slime", "up", 2, 0.2)
  walk_wait("slime", "right", 1, 0.2)
  walk_wait("slime", "up", 1, 0.2)
  walk_wait("slime", "right", 2, 0.2)
  walk_wait("slime", "down", 3, 0.2)
  walk_wait("slime", "left", 5, 0.2)
  walk_wait("slime", "up", 2, 0.2)
  walk_wait("slime", "left", 1, 0.2)
  walk_wait("slime", "up", 3, 0.2)
  walk_wait("slime", "right", 2, 0.2)
  walk_wait("slime", "up", 2, 0.2)
  walk_wait("slime", "left", 2, 0.2)
  walk_wait("slime", "down", 1, 0.2)
  walk_wait("slime", "left", 1, 0.2)
end

--#
