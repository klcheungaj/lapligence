// SIM-099 regression: semaphore methods called on a semaphore property
// selected through class handles (`h.s.get`, `a.b.s.put`, `h.s.try_get`)
// act on that semaphore (IEEE 1800-2009 15.3), from class methods and from
// module code. try_get() returns "a positive integer" on success (15.3.4),
// so the lines print whether it was positive.
`timescale 1ns / 1ns
class holder_c;
  semaphore s;
  function new(int keys);
    s = new(keys);
  endfunction
endclass

class outer_c;
  holder_c inner;
  function new();
    inner = new(0);
  endfunction
endclass

class user_c;
  holder_c h;
  outer_c o;
  function new(holder_c h, outer_c o);
    this.h = h;
    this.o = o;
  endfunction
  task take_two();
    h.s.get(2);
    $display("[%0t] user took two keys", $time);
  endtask
  task take_nested();
    o.inner.s.get();
    $display("[%0t] user took the nested key", $time);
  endtask
  function int try_one();
    return h.s.try_get(1);
  endfunction
endclass

module tb;
  holder_c hh;
  outer_c oo;
  user_c u;
  initial begin
    hh = new(1);
    oo = new;
    u = new(hh, oo);
    fork
      u.take_two();
      u.take_nested();
    join_none
    #2 hh.s.put(1);
    #2 oo.inner.s.put();
    #2 $display("[%0t] try_get on empty: %0d", $time, u.try_one() > 0);
    hh.s.put(3);
    $display("[%0t] try_get after put: %0d", $time, u.try_one() > 0);
    $display("[%0t] try_get of three: %0d", $time, u.h.s.try_get(3) > 0);
    $display("[%0t] try_get of two: %0d", $time, u.h.s.try_get(2) > 0);
    $finish;
  end
endmodule
