// S30-D8: a negative dist weight makes randomize fail with an error.
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
