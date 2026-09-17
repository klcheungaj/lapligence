// Static-review counterexample; NOT EXECUTED.
module tb;
  logic clk = 0;
  a: assert property (@(posedge clk) 1'b0) else $display("handled");
  b: assert property (@(posedge clk) 1'b0) else ;
  initial begin #1; clk = 1; #1; $finish(0); end
endmodule
// Expected application output: handled
// Neither a nor b requests the default $error action.
