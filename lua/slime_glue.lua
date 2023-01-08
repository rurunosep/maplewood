--# start

map_overlay_color(0, 0, 0, 255, 0)
lock_movement()
set_collision("player", false)

-- wait(1)
message("Previously, in v0.1.0...")
message("You broke into the house, got the plushy, and \n"
  .. "went to sleep.")

map_overlay_color(0, 0, 0, 0, 3)
wait(4)

message("\"... ... ...\"")
message("\"Wake up!\"")
walk("player", "down", 1)
message("(broke shit, etc)")
walk("man", "left", 4)
wait(0.5)
walk("player", "left", 0)
wait(0.5)
walk("man", "right", 0)
message("(put away plush)")

unlock_movement()
set_collision("player", true)

--#