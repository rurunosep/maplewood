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
-- player jump
walk("player", "down", 1, 0.15)
wait(0.5)

message("(broke shit, etc)")
message("(gotta take resposibility)")

-- player shake
-- wait

message("(it's ok, I got spaghetti)")

-- player jump
-- wait

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
-- jump slime
walk("slime", "right", 1, 0.15)
wait(0.5)

-- jump man
-- short wait

walk("player", "left", 0, 1)
-- jump player
-- short wait

walk("slime", "right", 0, 1)
wait(0.5)

message("(a slime!)")
message("(dead slimes make great glue!)")
-- jump slime
wait(0.5)
message("(catch it and kill it to fix the pot!)")
-- jump slime
wait(0.5)

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
wait(1)
remove_dead_sprite("player")
unlock_movement()
set("can_touch_slime", 1)

while(get("times_touched_slime") < 2) do
  coroutine.yield()
end

set("slime_loop", 0)
walk("slime", "down", 0, 1)
set_dead_sprite("slime", 16, 32)
set_cutscene_border()
lock_movement()
message("(you got it!)")
-- jump girl
wait(0.5)
message("(now bring here to pot)")
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
-- squish sounds
-- wait
set_cell_tile(12, 9, 2, 27)
wait(0.5)

message("(great)")
walk("player", "right", 0, 1)
message("(now come inside and wait for me to get \n"
  .. "the spaghetti)")
-- jump girl

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
-- jump girl
map_overlay_color(0, 0, 0, 255, 3)
-- play music
wait(3)
-- show card
wait(8)
-- remove card
wait(2)
close_game()


--# slime_loop

while(true) do
  walk("slime", "right", 5, 0.15)
  wait(2)
  wait_until_not_walking("slime")
  walk("slime", "left", 5, 0.15)
  wait(2)
  wait_until_not_walking("slime")
end

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

--#