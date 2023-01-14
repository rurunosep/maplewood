--# start

map_overlay_color(0, 0, 0, 255, 0)
lock_movement()
set_collision("player", false)

message("Previously, in v0.1.0...")
message("You broke into the house, got the plushy, and \n"
  .. "went to sleep.")

map_overlay_color(0, 0, 0, 0, 3)
wait(4)

message("\"... ... ...\"")
message("\"Wake up!\"")

add_position("slime", 6.5, 6.5)
set("slime_loop", 1)

walk("player", "down", 1, 0.2)
wait(0.5)
message("(broke shit, etc)")
walk("man", "left", 4, 0.06)
wait(0.5)
walk("player", "left", 0, 1)
wait_until_not_walking("man")
wait(0.5)
walk("man", "right", 0, 0)
wait(0.5)
message("(put away plush)")

unlock_movement()
set_collision("player", true)

--# slime_loop

while(true) do
  walk("slime", "right", 3, 0.2)
  wait_until_not_walking("slime")
  walk("slime", "left", 3, 0.2)
  wait_until_not_walking("slime")
end

--# slime_collision

set("slime_collided", 1)

set("slime_loop", 0)
walk("slime", "left", 0, 1)
set_dead_sprite("slime", 16, 32)

set_dead_sprite("player", 32, 0)
lock_movement()
wait(1)
remove_dead_sprite("player")
unlock_movement()

--#