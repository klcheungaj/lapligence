// Overridden nonblocking assignments: each always block below queues more
// than one update of a variable in one process turn, and only the last one
// determines the value (IEEE 1800-2009 10.4.2).
`timescale 1ns/1ns
module tb;
  logic clk;
  logic c, a, b;
  logic [1:0] sel;
  logic [3:0] m;
  logic [3:0] q, r, s, t, u, v;
  logic [7:0] w, w2, w3, w4;
  int n_t;
  int i;

  // Default, then an override under a condition.
  always @(posedge clk) begin s <= 4'd0; if (c) s <= s + 4'd1; end

  // An update of another variable between two updates of q: r reads q
  // before either update of q commits.
  always @(posedge clk) begin q <= 4'd1; r <= q + 4'd1; q <= 4'd2 + r; end

  // t returns to its value within one batch: no change, no wake.
  always @(posedge clk) begin t <= 4'd5; u <= u + 4'd1; t <= 4'd2; end
  always @(t) n_t = n_t + 1;

  // Adjacent updates, nested branches, a case without a default arm, a
  // loop with a run-time bit index and a partial update between two whole
  // updates.
  always @(posedge clk) begin
    w <= 8'h00; w <= 8'hff;
    w2 <= 8'd1; if (a) begin if (b) w2 <= 8'd2; else w2 <= 8'd3; end
    w3 <= 8'd9;
    case (sel) 2'd0: w3 <= 8'd10; 2'd1: ; 2'd2: w3 <= 8'd12; endcase
    for (i = 0; i < 4; i++) begin v[i] <= 1'b0; if (m[i]) v[i] <= 1'b1; end
    w4 <= 8'd7; w4[0] <= 1'b0; w4 <= w4 + 8'd1;
  end

  always @(negedge clk)
    $display("%0t q=%0d r=%0d s=%0d t=%0d u=%0d w=%0h w2=%0d w3=%0d v=%b w4=%0d n_t=%0d",
             $time, q, r, s, t, u, w, w2, w3, v, w4, n_t);

  initial begin
    clk = 0; c = 0; a = 0; b = 0; sel = 0; m = 4'b1010;
    q = 0; r = 0; s = 0; t = 4'd2; u = 0; v = 0;
    w = 0; w2 = 0; w3 = 0; w4 = 0; n_t = 0;
    #1 clk = 1; #1 clk = 0; c = 1; a = 1; sel = 1; m = 4'b0101;
    #1 clk = 1; #1 clk = 0; b = 1; sel = 2;
    #1 clk = 1; #1 clk = 0; c = 0; a = 0; sel = 3;
    #1 clk = 1; #1 clk = 0;
    #1 $finish(0);
  end
endmodule
