// SIM-009-A03: SV 13.4 a) forbids time-controlled statements in a function;
// the recursive form of this helper must be a task.
module tb;
  function automatic int down(int n);
    #1;
    return n == 0 ? 0 : down(n - 1);
  endfunction
  initial $display("%0d", down(2));
endmodule
