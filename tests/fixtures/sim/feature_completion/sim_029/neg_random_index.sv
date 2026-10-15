// S29-D7: a select whose index is a random variable is rejected.
class c_c; rand bit [7:0] a; rand bit [2:0] i; constraint s { a[i] == 1'b1; } endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
