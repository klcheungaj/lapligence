// S30-D10: a dist weight or item bound that reads a random variable is
// rejected.
class c_c; rand bit [3:0] x, w; constraint d { x dist { 1 := w, 2 := 1 }; } endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
