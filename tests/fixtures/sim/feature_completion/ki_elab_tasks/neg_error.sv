// $error (IEEE 1800-2009 20.10): every call reports and no simulation runs.
`timescale 1ns/1ps
module sub #(parameter int W = 8) ();
  for (genvar I = 0; I < 2; I++) begin : g
    if (I < W) begin : bad
      $error("I=%0d W=%0d", I, W);
    end
  end
endmodule
module tb;
  sub #(1) u();
  sub #(1) v();
  initial $display("must not run");
endmodule
