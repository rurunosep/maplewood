function wrap_yielding(f)
  return function(...)
    f(...)
    line_yielded_at = current_line(2)
    coroutine.yield()
  end
end

function yield()
  line_yielded_at = current_line(2)
  coroutine.yield()
end

function walk_wait(entity, direction, distance, speed)
  walk(entity, direction, distance, speed)
  wait_until_not_pathing(entity)
end

function walk_to_wait(entity, x, y, speed)
  walk_to(entity, x, y, speed)
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
