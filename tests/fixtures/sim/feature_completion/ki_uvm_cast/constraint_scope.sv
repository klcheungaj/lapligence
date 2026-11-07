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
module tb;
  S s = new;
  initial begin
    s.only_alt();
    $display("%0d", s.mode());
    if (s.randomize()) $display("%0d", s.x > 6);
    s.alt.constraint_mode(0);
    s.only_pick();
    $display("%0d", s.mode());
    if (s.randomize()) $display("%0d", s.x < 3);
    $finish;
  end
endmodule
