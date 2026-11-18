// Overridden nonblocking assignments seen by $monitor, $strobe and event
// controls: a value that returns within one NBA batch is no change, and a
// report shows the last value (IEEE 1800-2009 10.4.2, 21.2.3).
`timescale 1ns/1ns
module tb;
  logic clk;
  logic [3:0] q, r, s, k;
  int n_q, n_k;

  always @(posedge clk) begin q <= 4'd1; r <= r + 4'd1; q <= 4'd2; end
  always @(posedge clk) begin s <= 4'd3; s <= 4'd4 + r; end
  always @(posedge clk) begin k <= 4'd6; k <= 4'd2; end
  always @(q) n_q = n_q + 1;
  always @(k) n_k = n_k + 1;

  initial $monitor("%0t monitor q=%0d r=%0d s=%0d", $time, q, r, s);
  always @(posedge clk) $strobe("%0t strobe q=%0d r=%0d s=%0d k=%0d", $time, q, r, s, k);

  initial begin
    clk = 0; q = 4'd2; r = 0; s = 0; k = 4'd2; n_q = 0; n_k = 0;
    #1 clk = 1; #1 clk = 0;
    #1 clk = 1; #1 clk = 0;
    #1 $display("%0t n_q=%0d n_k=%0d q=%0d r=%0d s=%0d k=%0d", $time, n_q, n_k, q, r, s, k);
    $finish(0);
  end
endmodule
