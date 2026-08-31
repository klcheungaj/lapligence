// llg-dump-fixture: tb.sv
module tb;
  logic wa;
  logic wb;
  logic t_q;

  m_a u_a(.clk(wa), .q(t_q));
  m_b u_b(.clk(wb));

  assign t_q = u_a.busy ^ wa ^ wb;
endmodule
