function wrap_yielding(f)
  return function(...)
    f(...)
    line_yielded_at = current_line(2)
    coroutine.yield()
  end
end

-- Because LDtk doesn't handle "\n" properly
nl = "\n"
