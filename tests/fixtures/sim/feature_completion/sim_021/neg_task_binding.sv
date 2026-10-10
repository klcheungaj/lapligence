// IEEE 1800-2009 11.11, 13.4 b): an overload binds a function; a task (which
// may consume time) is not a function and cannot be bound.
module tb;
  typedef struct { int v; string n; } t_t;
  task automatic fa(t_t x, t_t y);
    #1;
  endtask
  bind + function t_t fa(t_t, t_t);
  t_t x, y, z;
  initial z = x + y;
endmodule
