local rocks_before = get_story_var("test.well.rocks_inside")
local rocks_thrown = 0

message("It's a shallow well")

if rocks_before > 0 then
    message("There are some rocks inside")
end

repeat
    local s = selection("Throw a rock in?\n 1: Yes\n2: No")
    if s == 1 then
        rocks_thrown = rocks_thrown + 1
        message("You throw a rock in")
    end
until s == 2

local rocks_after = rocks_before + rocks_thrown
if rocks_thrown == 0 then
    message("You leave without throwing in any rocks")
else
    message(string.format("You leave after throwing in %d rocks", rocks_thrown))
    message(string.format("There are now %d rocks in the well", rocks_after))
end
set_story_var("test.well.rocks_inside", rocks_after)
