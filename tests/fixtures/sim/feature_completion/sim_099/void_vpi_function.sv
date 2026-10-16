// SIM-099 regression: a user-defined VPI system function in a void cast
// (IEEE 1800-2009 13.4.1) is called as a function: sim_042's systf.c
// registers $sum as a sized signed function whose calltf prints the call's
// line, result size and function type.
module tb;
  initial begin
    void'($sum(8'd3, 12'sd4));
    $display("after void sum");
    $finish;
  end
endmodule
