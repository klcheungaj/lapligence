// Decision A19-D54: the index and implicit declaration of generate scopes.
`timescale 1ns/1ns
module tb;
  genvar k;
  for (k = 2; k < 4; k++) begin : blk
    wire w;
  end
  for (k = -1; k < 1; k++) begin
    wire u;
  end
  if (1) begin
    wire v;
  end
  if (1) begin : named
    wire z;
    for (genvar j = 0; j < 1; j++) begin
      wire q;
    end
  end
  initial #1 $d54;
endmodule
