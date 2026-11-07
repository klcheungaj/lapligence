class S;
  rand int x;
  constraint pick { x inside {1, 2}; }
  constraint alt { x inside {7, 8}; }
  function void only_alt();
    this.pick.constraint_mode(0);
  endfunction
  function void only_pick();
    this.alt.constraint_mode(0);
    this.pick.constraint_mode(1);
  endfunction
  function int mode();
    return this.pick.constraint_mode() * 10 + this.alt.constraint_mode();
  endfunction
endclass
class T extends S;
  static int count = 0;
  constraint third { x != 3; }
  function void bare_off();
    pick.constraint_mode(0);
    alt.constraint_mode(0);
    third.constraint_mode(0);
  endfunction
  task bare_on();
    #1 third.constraint_mode(1);
    pick.constraint_mode(1);
  endtask
  function int bare_modes();
    return pick.constraint_mode() * 100 + alt.constraint_mode() * 10
      + third.constraint_mode();
  endfunction
endclass
module tb;
  S s = new;
  T t = new;
  initial begin
    s.only_alt();
    $display("%0d", s.mode());
    if (s.randomize()) $display("%0d", s.x > 6);
    s.alt.constraint_mode(0);
    s.only_pick();
    $display("%0d", s.mode());
    if (s.randomize()) $display("%0d", s.x < 3);
    t.bare_off();
    $display("%0d", t.bare_modes());
    t.bare_on();
    $display("%0d", t.bare_modes());
    if (t.randomize()) $display("%0d", t.x < 3);
    $finish;
  end
endmodule
