// FIN-002 gate-level negative: extended VCD port dumping is not simulated.
`timescale 1ns / 1ns
module cell_buf (
    output y,
    input  a
);
  buf (y, a);
endmodule
module tb;
  reg a;
  wire y;
  cell_buf u (
      .y(y),
      .a(a)
  );
  initial begin
    $dumpports(u, "ports.evcd");
    a = 1'b0;
    #1 a = 1'b1;
    #1 $finish;
  end
endmodule
