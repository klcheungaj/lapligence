// llg-dump-fixture: m_a.sv
module m_a(input logic clk, output logic q);
  logic busy;
  assign q = clk & ~busy;
endmodule
