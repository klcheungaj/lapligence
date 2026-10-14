// solve...before is planned for SIM-030 and must not be dropped.
class c_c; rand bit a; rand bit [3:0] b; constraint o { solve a before b; } endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
