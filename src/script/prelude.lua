-- Set line_yielded_at to the line where the caller of this function was
-- called, only if it has not already been set.
-- Any function that yields should call this before it calls coroutine.yield()
-- or another subfunction that yields, so that line_yielded_at may be properly
-- set to the line in the script where the first yielding function was called.
function set_line_yielded_at()
  if line_yielded_at == nil then
    line_yielded_at = current_line(2)
  end
end

-- Clear line_yielded at only if it is equal to the line where the caller of
-- this function was called.
-- Any function that yields should call this after it calls coroutine.yield()
-- or another subfunction that yields, so that line_yielded_at may be properly
-- cleared when execution return to the line in the script where the first yielding
-- function was called.
-- (This will fail if, by chance, calls in different files have the same line number.)
function clear_line_yielded_at()
  if line_yielded_at == current_line(2) then
    line_yielded_at = nil
  end
end

-- Convert a normal function into one that does the same thing then yields
function wrap_yielding(f)
  return function(...)
    f(...)
    set_line_yielded_at()
    coroutine.yield()
    clear_line_yielded_at()
  end
end

function yield()
  set_line_yielded_at()
  coroutine.yield()
  clear_line_yielded_at()
end

function walk_wait(entity, direction, distance, speed)
  walk(entity, direction, distance, speed)
  set_line_yielded_at()
  wait_until_not_pathing(entity)
  clear_line_yielded_at()
end

function walk_to_wait(entity, x, y, speed)
  walk_to(entity, x, y, speed)
  set_line_yielded_at()
  wait_until_not_pathing(entity)
  clear_line_yielded_at()
end

function wait_until_not_pathing(entity)
  while (is_entity_pathing(entity)) do
    set_line_yielded_at()
    coroutine.yield()
    clear_line_yielded_at()
  end
end

-- Because LDtk doesn't handle "\n" properly
nl = "\n"
