// SIM-024: `%m` takes no field width (SV 21.2.1.6).
module tb;
  initial $display("%10m");
endmodule
