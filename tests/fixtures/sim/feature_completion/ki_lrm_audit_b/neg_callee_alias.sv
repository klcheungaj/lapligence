// A function reads a class property through a local that holds one of two
// handles depending on a run-time choice; llg binds a callee local only when
// every write stores the same re-evaluable handle.
module tb;
  class C;
    int x;
  endclass
  C h = new;
  C g = new;
  function automatic int pick_x(C a, C b, bit sel);
    C d;
    if (sel) d = a;
    else d = b;
    return d.x;
  endfunction
  initial begin
    wait (pick_x(h, g, 1'b1) == 1);
    $display("woke");
  end
  initial #1 h.x = 1;
endmodule
