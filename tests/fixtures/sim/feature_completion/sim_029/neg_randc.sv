// randc is planned for SIM-030 and must not be silently treated as rand.
class c_c; randc bit [3:0] r; endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
