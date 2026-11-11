// A randc array needs one cycle per element, which has no storage yet: an
// explicit error, never elements drawn as plain rand.
class c_c;
  randc bit [2:0] a [4];
endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
