// Random object members (global constraints) are planned for SIM-030.
class leaf_c; rand bit [3:0] v; endclass
class c_c; rand leaf_c l; endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
