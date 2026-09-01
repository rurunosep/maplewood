function wrap_yielding(f)
  return function(...)
    f(...)
    line_yielded_at = current_line(2)
    coroutine.yield()
  end
end

function path_to_wait(entity, x, y)
  path_to(entity, x, y)
  wait_until_not_pathing(entity)
end

function path_rel_wait(entity, direction, distance)
  path_rel(entity, direction, distance)
  wait_until_not_pathing(entity)
end

function wait_until_not_pathing(entity)
  while (is_entity_pathing(entity)) do
    line_yielded_at = current_line(3)
    coroutine.yield()
  end
end

-- Because LDtk doesn't handle "\n" properly
nl = "\n"
