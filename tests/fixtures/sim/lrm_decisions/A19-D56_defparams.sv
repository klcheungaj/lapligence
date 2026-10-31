// Decision A19-D56: defparams (IEEE 1800-2009 23.10.1, 37.26, 37.80).
// Companion of A19-D56_defparams.c.
`timescale 1ns/1ns
module leaf;
  parameter Q = 1;
  parameter [7:0] R = 8'h0f;
endmodule
module mid;
  parameter P = 0;
  leaf u ();
  defparam u.R = 8'h33;
endmodule
module tb;
  mid t ();
  mid m ();
  defparam t.P = 5;
  defparam t.u.Q = 2 + 1, m.P = -4;
  if (1) begin : g
    leaf v ();
    defparam v.Q = 7;
  end
  initial #1 $d56;
endmodule
