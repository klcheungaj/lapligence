// Static-review counterexample; NOT EXECUTED.
program p_a;
  initial begin #1; $exit; $display("ERROR: p_a continued"); end
endprogram
program p_b;
  initial begin #3; $display("p_b survived p_a exit"); end
endprogram
module tb;
  p_a a();
  p_b b();
endmodule
