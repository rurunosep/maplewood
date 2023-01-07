--# 1
message("I'm going inside")
teleport_entity('skele_1', 8.5, 7.5)

--# 2
message('Yo')

--# 2b
while (true)
do
  walk("skele_2", "left", 3)
  while (table.pack(get_entity_position("skele_2"))[1] > 8.5)
  do
    coroutine.yield()
  end

  walk("skele_2", "right", 3)
  while (table.pack(get_entity_position("skele_2"))[1] < 11.5)
  do
    coroutine.yield()
  end
end

--# 3
message('Sup')

--#