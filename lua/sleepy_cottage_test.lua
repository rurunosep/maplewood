--# start
message("You were taking a walk in the woods, \n"
    .. "but you're sooooo sleepy.")
message("You need someplace to take a nap.")

--# sign
if is_player_at_cellpos(7, 11) then
    message("\"Welcome!\"")
else
    message("That's the wrong side.")
end

--# grave
if get("got_plushy") == 1 then
    if get("tried_to_leave_plushy") == 1 then
        message("Just get to bed.")
    else
        local s = selection("Leave Bobo at the grave?\n1: Yes\n2: No")
        if s == 1 then
            message("That's nice.")
            message("But you need him more.")
            set("tried_to_leave_plushy", 1)
        end
    end
end
if get("read_grave_note") == 0 then
    message("There's an old note by the grave:")
    message("\"To my dearly departed:\"")
    message("\"If you ever rise from your slumber and want to \n"
        .. "come inside, the key to the front door is in the pot \n"
        .. "in our garden.\"")
    set("read_grave_note", 1);
end

--# pot
if get("read_grave_note") == 1 and get("got_door_key") == 0 then
    message("The key should be in this pot.")
    local s = selection("Carefully pull out the key?\n1: Yes\n2: No")
    if s == 1 then
        play_sfx("smash_pot")
        set_cell_tile(12, 9, 2, 28)
        message("You got the key!")
        set("got_door_key", 1);
    end
end

--# door
if get("got_door_key") == 1 then
    if get("opened_door") == 0 then
        play_sfx("door_open")
        set_cell_tile(8, 8, 2, -1)
        set_cell_passable(8, 8, true)
        message("You're in!")
        set("opened_door", 1)
    end
else
    message("It's locked shut.")
end

--# door_collision
if get("read_dresser_note") == 1 and get("burned_dresser_note") == 0 then
    play_sfx("door_close")
    set_cell_tile(8, 8, 2, 48)
    set_cell_passable(8, 8, false)
    message("Burn after reading!")
end

--# bed
if get("got_plushy") == 1 then
    local s = selection("Go to sleep?\n1: Yes\n2: No")
    if s == 1 then
        message("Finally!")
        message("Sleep tight (:")
        play_music("sleep", false)
        lock_movement()
        fade_to_black(5)
        wait(12)
        close_game()
    end
elseif get("tried_to_sleep") == 1 then
    message("You need a plushy!")
else
    message("You can't go to sleep without a plushy")
    set("tried_to_sleep", 1)
end

--# dresser
if get("tried_to_sleep") == 1 and get("read_dresser_note") == 0 then
    message("There's a note in one of the drawers:")
    message("\"I keep my special bedtime friend safe in the chest \n"
        .. "during the day.\"")
    message("\"The key is hidden in the tree next to the well \n"
        .. "outside.\"")
    message("\"Burn after reading!\"")
    message("That's a weird note to leave in your dresser.")
    set("read_dresser_note", 1)
end

--# brazier
if get("read_dresser_note") == 1 and get("burned_dresser_note") == 0 then
    local s = selection("Burn the note?\n1: Yes\n2: No")
    if s == 1 then
        play_sfx("flame")
        set_cell_tile(8, 8, 2, -1)
        set_cell_passable(8, 8, true)
        message("The secret dies with you.")
        set("burned_dresser_note", 1)
    end
end
if get("got_plushy") == 1 then
    if get("tried_to_burn_plushy") == 1 then
        message("No!")
    else
        local s = selection("Burn Bobo?\n1: Yes\n2: No")
        if s == 1 then
            message("You could never! D:")
            set("tried_to_burn_plushy", 1)
        end
    end
end

--# tree
if get("read_dresser_note") == 1 and get("got_chest_key") == 0 then
    message("You find a key hidden amongst the leaves!")
    play_sfx("drop_in_water")
    message("It fell in the well!")
    message("...")
    message("Just kidding  (:")
    message("You have the key safe in your hand.")
    set("got_chest_key", 1)
end

--# chest
if get("got_chest_key") == 1 and get("got_plushy") == 0 then
    play_sfx("chest_open)
    set_cell_tile(8, 5, 2, 35)
    message("You got the chest open!")
    message("There's a plushy inside! \n"
        .. "The stitching reads \"BOBO\".")
    message("He's so soft (:")
    message("It would be a shame if anything happened to him.")
    set("got_plushy", 1)
end

--# well
if get("got_plushy") == 1 then
    if get("tried_to_drown_plushy") == 1 then
        message("No!")
    else
        local s = selection("Drown Bobo?\n1: Yes\n2: No")
        if s == 1 then
            message("You could never! D:")
            set("tried_to_drown_plushy", 1)
        end
    end
end

--# stairs_collision
force_move_player_to_cell("down", 6, 6)
message("That's trespassing.")

--#