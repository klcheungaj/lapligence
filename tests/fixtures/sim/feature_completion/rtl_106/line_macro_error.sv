`define BAD_USE(x) x = missing_signal
module tb;
  logic a;
`line 200 "gen_top.sv" 0
  initial begin
    `BAD_USE(a);
  end
endmodule
