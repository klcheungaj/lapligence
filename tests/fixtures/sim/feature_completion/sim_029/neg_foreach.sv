// foreach constraints are planned for SIM-030.
class c_c; bit [3:0] v [4]; rand bit [3:0] x; constraint f { foreach (v[i]) x != v[i]; } endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
