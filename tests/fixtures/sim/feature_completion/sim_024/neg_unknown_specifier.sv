// SIM-024: `%q` is not a format specification (SV 21.2.1.2, Table 21-1).
module tb;
  initial $display("%q", 1);
endmodule
