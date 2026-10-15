// A random divisor is planned for SIM-030.
class c_c; rand bit [7:0] a, b; constraint q { a / b == 3; } endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
