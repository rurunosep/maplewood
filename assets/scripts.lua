--# start

message("You're sleepy.")
message("But you need a plushy.")
message("Legend tells that the kid in the classroom has a plushy.")

--# kid

local stages = {
  [1] = function()
    message("You need a plushy?\n" ..
      "I have one.\n" ..
      "But I need your help.")
    message("I skipped class yesterday, and I need you to write my\n" ..
      "name in the attendance book.")
    message("Get the teacher's pen from the toilet.")
    set_story_var("school::kid::stage", 2)
  end,
  [2] = function()
    message("I'll tell you where the plushy is when you put my name\n" ..
      "in the book.")
  end,
  [3] = function()
    message("Thanks a lot!")
    message("The plushy is in the gym.")
    set_story_var("school::kid::stage", 4)
  end,
  [4] = function()
    message("The plushy is in the gym.")
  end
}

stages[get_story_var("school::kid::stage")]()

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
    set_story_var("gym::janitor::stage", 3)
    set_story_var("bakery::girl::stage", 2)
  end,
  [3] = function()
    message("I need that bun.")
  end,
  [4] = function()
    message("Thanks a bunch! Now I can run.")
    message("Here's the key.")
    set_story_var("gym::janitor::stage", 5)
    set_story_var("bathroom::door::have_key", 1)
  end,
  [5] = function()
    message("Now I can run.")
  end
}

stages[get_story_var("gym::janitor::stage")]()

--# bakery_girl

local stages = {
  [1] = function()
    message("I sell buns.")
  end,
  [2] = function()
    message("You need a Super Sugar Bun?")
    message("That's 25 cents.")
    message("You can get a quarter by returning a shopping cart.")
    set_story_var("bakery::girl::stage", 3)
  end,
  [3] = function()
    message("Get the quarter.")
  end,
  [4] = function()
    message("Okay, here's the bun.")
    set_story_var("bakery::girl::stage", 5)
    set_story_var("gym::janitor::stage", 4)
  end,
  [5] = function()
    message("Have a nice day.")
  end
}

stages[get_story_var("bakery::girl::stage")]()

--#
