// S30-D14 (18.7, 18.7.1): in an inline constraint on another object of the
// caller's class, an unqualified name and `this` are the randomized object's;
// `local::` names the caller's, even when both are one declaration.
class c_c;
  rand int x;
  int y = 3;
  function void go(c_c o);
    x = 50;
    y = 4;
    $display("%0d %0d", o.randomize() with { x == y + 1; }, o.x);
    $display("%0d %0d", o.randomize() with { x == local::x + 1; }, o.x);
    $display("%0d %0d", o.randomize() with { x == local::y; }, o.x);
    $display("%0d %0d", o.randomize() with { x == this.y; }, o.x);
    $display("%0d %0d", randomize() with { x == local::y + 1; }, x);
  endfunction
endclass

class d_c extends c_c;
  function void go2(c_c o);
    $display("%0d %0d", o.randomize() with { x == 9; }, o.x);
  endfunction
endclass

module tb;
  initial begin
    static c_c a = new;
    static c_c b = new;
    static d_c d = new;
    b.y = 7;
    a.go(b);
    d.go2(b);
    $finish(0);
  end
endmodule
