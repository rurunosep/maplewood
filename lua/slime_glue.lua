--# start

map_overlay_color(0, 0, 0, 255, 0)
lock_movement()
set_collision("player", false)
set_dead_sprite("player", 32, 16)
set_cutscene_border()

message("Previously, in v0.1.0...")
message("You broke into the house, got the plushy, and \n"
  .. "went to sleep.")

map_overlay_color(0, 0, 0, 0, 3)
wait(4)

message("\"... ... ...\"")
message("\"Wake up!\"")

remove_dead_sprite("player")
jump("player")
walk("player", "down", 1, 0.15)
wait(1)

message("(broke shit, etc)")
message("(gotta take resposibility)")

quiver("player", 0.8)
wait(1)

message("(it's ok, I got spaghetti)")

jump("player")
wait(0.3)
jump("player")
wait(0.3)

walk("man", "left", 4, 0.06)
wait(0.7)
walk("player", "left", 0, 1)
wait_until_not_walking("man")
wait(0.5)

walk("man", "right", 0, 1)
wait(0.5)

message("(first of all, put away plush)")

unlock_movement()
set_collision("player", true)
remove_cutscene_border()

-- start script for man to keep looking at player

while(get("put_away_plushy") == 0) do
  coroutine.yield()
end

walk_to("player", "down", 6.5, 0.06)
wait_until_not_walking("player")
walk_to("player", "left", 8.5, 0.06)
wait_until_not_walking("player")
walk_to("player", "right", 8.5, 0.06)
wait_until_not_walking("player")
walk_to("player", "up", 6.5, 0.06)
wait_until_not_walking("player")

set_cell_tile(8, 5, 2, 36)
-- end man look at player script
set_cutscene_border()
lock_movement()
wait(0.5)

message("(good, now follow me outside)")
set_cell_tile(8, 8, 2, -1)
set_cell_passable(8, 8, true)
walk_to("man", "down", 10.5, 0.06)
wait(0.7)
walk("player", "down", 4, 0.06)
-- door closes when player passes it?
wait_until_not_walking("man")
walk("man", "right", 4, 0.06)
wait_until_not_walking("player")
walk("player", "right", 0, 1)
wait_until_not_walking("man")
walk("man", "up", 0, 1)
wait(0.5)
message("(why even)")
wait(0.5)
walk("man", "left", 0, 1)
wait(0.5)
message("(need some way to fix this)")

add_position("slime", 5.5, 11.5)
jump("slime")
walk("slime", "up", 1, 0.15)
wait(0.8)

jump("man")
wait(0.8)

walk("player", "left", 0, 1)
jump("player")
wait(1)

message("(it's a slime!)")
walk("slime", "right", 0, 1)
wait(1)
message("(dead slimes make great glue!)")
jump("slime")
wait(1)
message("(catch it and kill it to fix the pot!)")
jump("slime")
wait(1)

-- play music
set("slime_loop", 1)
set_collision("slime", true)
remove_cutscene_border()
unlock_movement()
set("can_touch_slime", 1)

while(get("times_touched_slime") < 1) do
  coroutine.yield()
end

lock_movement()
set_dead_sprite("player", 32, 0)
play_sfx("slip")
wait(1)
remove_dead_sprite("player")
unlock_movement()
set("can_touch_slime", 1)

while(get("times_touched_slime") < 2) do
  coroutine.yield()
end

-- stop music
play_sfx("squish")
set("slime_loop", 0)
walk("slime", "down", 0, 1)
set_dead_sprite("slime", 16, 32)
set_cutscene_border()
lock_movement()
message("(you got it!)")
jump("player")
wait(0.3)
jump("player")
wait(0.3)
wait(0.5)
message("(now bring it here to pot)")
walk("man", "right", 1, 0.06)
wait_until_not_walking("man")
walk("man", "left", 0, 1)
remove_position("slime")
remove_cutscene_border()
unlock_movement()

while(get("fixed_pot") == 0) do
  coroutine.yield()
end

set_cutscene_border()
lock_movement()
play_sfx("squish")
wait(0.8)
play_sfx("squish")
wait(0.8)
play_sfx("squish")
wait(0.8)
set_cell_tile(12, 9, 2, 27)
wait(1)

message("(great)")
walk("player", "right", 0, 1)
message("(now come inside and wait for me to get \n"
  .. "the spaghetti)")
jump("player")

walk("man", "down", 1, 0.06)
wait_until_not_walking("man")
walk("man", "left", 5, 0.06)
wait(0.5)
walk("player", "down", 0, 1)
wait_until_not_walking("man")
walk("player", "left", 0, 1)
walk("man", "up", 5, 0.06)
wait_until_not_walking("man")
set("door_may_close", 1)
remove_cutscene_border()
unlock_movement()
walk("man", "left", 2, 0.06)
wait_until_not_walking("man")
walk("man", "up", 1, 0.06)
wait_until_not_walking("man")
remove_position("man")

while(get("door_may_close") == 1) do
  coroutine.yield()
end

wait(3)

add_position("man", 6.5, 5.5)
walk("man", "down", 0, 1)
set_cutscene_border()
lock_movement()
message("(I got the spaghetti!)")
jump("player")
wait(0.3)
jump("player")
wait(0.3)
jump("player")
wait(0.3)
wait(0.5)
map_overlay_color(0, 0, 0, 255, 3)
-- play music
wait(3)
show_card()
wait(8)
remove_card()
wait(2)
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

--# slime_loop

while(true) do
  walk("slime", "left", 2, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "down", 2, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "right", 1, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "down", 3, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "right", 3, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "down", 2, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "left", 1, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "down", 2, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "right", 3, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "up", 1, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "right", 2, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "up", 3, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "left", 1, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "up", 2, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "left", 4, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "down", 3, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "right", 3, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "up", 2, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "right", 1, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "up", 1, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "right", 2, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "down", 3, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "left", 5, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "up", 2, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "left", 1, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "up", 3, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "right", 2, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "up", 2, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "left", 2, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "down", 1, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "left", 1, 0.2)
  wait_until_not_walking("slime")
end

--#