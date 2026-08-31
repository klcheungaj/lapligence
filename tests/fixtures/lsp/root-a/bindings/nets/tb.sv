// llg-lsp-fixture: root-a/bindings/nets/tb.sv
module tb;
  logic wa;
  logic wb;

  m_a ua(.clk(wa));
  m_b ub(.clk(wb));

  always #5 wa = ~wa;
  always #10 wb = ~wb;
endmodule
