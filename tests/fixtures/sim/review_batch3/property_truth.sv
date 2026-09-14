// Static-review regression; not executed during patch preparation.
`timescale 1ns/1ps
module tb;
  bit clk = 0;
  logic unknown_value = 1'bx;
  logic high_impedance = 1'bz;
  logic [1:0] known_nonzero = 2'b1x;
  int successes = 0;
  int failures = 0;
  a: assert property (@(posedge clk) not unknown_value) successes++; else failures++;
  b: assert property (@(posedge clk) not high_impedance) successes++; else failures++;
  c: assert property (@(posedge clk) if (unknown_value) 1'b0 else 1'b1) successes++; else failures++;
  d: assert property (@(posedge clk) unknown_value implies 1'b0) successes++; else failures++;
  e: assert property (@(posedge clk) unknown_value iff high_impedance) successes++; else failures++;
  f: assert property (@(posedge clk) known_nonzero) successes++; else failures++;
  initial begin
    if ((!unknown_value) !== 1'bx) $fatal(1, "expression logic must remain four-state");
    #1 clk = 1;
    #1;
    if (successes != 6 || failures != 0) $fatal(1, "property truth conversion");
    $display("property truth ok");
    $finish(0);
  end
endmodule
