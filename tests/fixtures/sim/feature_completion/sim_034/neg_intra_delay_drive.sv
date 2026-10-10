// SV 1800-2009 14.16: no intra-assignment delay other than a cycle delay is
// legal in a synchronous drive (`bus.data <= #4 r; // error`).
module tb;
  bit clk;
  int x;
  clocking cb @(posedge clk);
    output x;
  endclocking
  initial begin
    cb.x <= #4 1;
    $finish;
  end
endmodule
