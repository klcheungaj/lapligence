// llg-dump-fixture: m_b.sv
module m_b(input logic clk);
  logic busy;
  assign busy = ~clk;
endmodule
