// `**` on a random variable is not bit-blasted.
class c_c; rand bit [3:0] a; constraint p { a ** 2 == 9; } endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
