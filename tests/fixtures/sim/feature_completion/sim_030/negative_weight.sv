// S30-D8: a dist weight is unsigned, so the `int` weight -1 is 2**32 - 1
// against 1 (x is 2 only with probability 2**-32).
class c_c;
  rand bit [3:0] x;
  int w = -1;
  constraint d { x dist { 1 := w, 2 := 1 }; }
endclass
module tb;
  initial begin
    static c_c c = new;
    int ok;
    c.x = 4'd7;
    ok = c.randomize();
    $display("ok=%0d x=%0d", ok, c.x);
    $finish;
  end
endmodule
