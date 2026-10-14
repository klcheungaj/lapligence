// A randomize() in a loop condition is evaluated on every iteration; it is
// rejected rather than hoisted.
class c_c; rand bit [3:0] a; constraint k { a > 3; } endclass
module tb;
  initial begin
    static c_c c = new;
    while (!c.randomize()) $display("retry");
    $finish;
  end
endmodule
