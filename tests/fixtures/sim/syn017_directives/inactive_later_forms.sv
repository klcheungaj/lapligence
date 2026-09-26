// IEEE 1364-2001 19.4: an untaken group has no directive effect.
module tb;
`ifdef USE_SV
  `define CAT(a,b) a``b
  `define STR(x) `"x`"
  `pragma protect
`endif
  initial begin
    $display("ok");
    $finish;
  end
endmodule
