// A function reads a class property through a local copy of its handle
// formal; llg cannot re-evaluate that handle when the wait arms.
module tb;
  class C;
    int x;
  endclass
  C h = new;
  function automatic int alias_x(C c);
    C d;
    d = c;
    return d.x;
  endfunction
  initial begin
    wait (alias_x(h) == 1);
    $display("woke");
  end
  initial #1 h.x = 1;
endmodule
