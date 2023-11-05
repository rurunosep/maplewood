--# grab_a_bun

lock_player_input()
set_cutscene_border()
set_entity_solid("player", false)
set_entity_map_pos("player", 10.5, 9.2)
wait(1)
walk_to_wait("player", "left", 2.5, 0.1)
walk_to_wait("player", "up", 4.5, 0.1)
wait(0.5)
message("You grab a bun.")
wait(0.5)
walk_to_wait("player", "down", 8.5, 0.1)
walk_to_wait("player", "right", 10.5, 0.1)
walk_to_wait("player", "down", 9.2, 0.1)
wait(0.5)
set_entity_map_pos("player", 10.5, 11.2)
wait(1)
set_entity_solid("player", true)
remove_cutscene_border()
unlock_player_input()

--#
