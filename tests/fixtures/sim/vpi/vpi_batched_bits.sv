// Delayed gates that drive the bits of one vector through merged selected
// output ports (A21-D5) and mature together commit as one batch: the vector
// changes once per batch, with every bit's new value (vpi_batched_bits.c),
// while each formal still changes once for its own bit.
`timescale 1ns/1ns
module stage(input d, output q);
  buf #2 b(q, d);
endmodule

module tb;
  reg d = 1'b0;
  wire [7:0] q;
  genvar i;
  generate
    for (i = 0; i < 8; i = i + 1) begin : g
      stage s(.d(d), .q(q[i]));
    end
  endgenerate
  initial begin
    #5 d = 1'b1;
    #5 d = 1'b0;
    #5 $finish;
  end
endmodule
