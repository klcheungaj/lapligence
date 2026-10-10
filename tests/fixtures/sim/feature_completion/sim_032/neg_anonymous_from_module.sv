// SIM-032 A03: anonymous program items are accessible only to programs
// (IEEE 1800-2009 24.6).
program;
  function int twice(int x);
    return 2 * x;
  endfunction
endprogram

module tb;
  initial $display("%0d", twice(2));
endmodule
