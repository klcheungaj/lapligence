// SIM-027 Q02 resolution: two-state words read X and Z digits as 0
// (IEEE 1800-2009 21.4.2), whichever of extension and conversion comes
// first, so the result is fixed. Expected values are derived by hand.
module tb;
  bit [7:0] hex8 [0:4];
  bit [3:0] bin4 [0:4];
  initial begin
    $readmemh("tokens.hex", hex8, 0, 4);
    $readmemb("tokens.bin", bin4, 0, 4);
    $display("hex8 %b %b %b %b %b", hex8[0], hex8[1], hex8[2], hex8[3], hex8[4]);
    $display("bin4 %b %b %b %b %b", bin4[0], bin4[1], bin4[2], bin4[3], bin4[4]);
    $finish;
  end
endmodule
