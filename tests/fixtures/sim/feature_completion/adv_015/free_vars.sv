// ADV-015b A02: free checker variables (IEEE 1800-2009 17.7, 17.7.2).
// Every printed value is forced by the clauses, whatever values the
// simulator draws: assumptions hold at every assume set clock event,
// assertions read the solved (current) values, a constant free variable
// keeps its random initial value, an initialized active variable keeps its
// initial value in the first time step, an assigned free variable is
// inactive, and implicitly or explicitly clocked unconstrained variables
// are redrawn.
`timescale 1ns / 1ns
checker observer_model(bit valid, bit reset, event clk);
  rand bit flag;
  rand bit [7:0] r;
  rand const bit [5:0] idx;
  rand bit [3:0] k = 4'd9;
  rand bit [15:0] w;
  rand bit t;
  bit [15:0] w_seen = 16'd0;
  bit [3:0] w_changes = 4'd0;
  m1: assume property (@clk reset |=> !flag);
  m2: assume property (@clk !reset && flag |=> flag);
  m3: assume property (@clk !valid |-> !flag);
  m4: assume property (@clk r inside {[8'd10:8'd20]});
  a1: assert property (@clk reset |=> !flag) else $display("%0t a1 failed", $time);
  a2: assert property (@clk !reset && flag |=> flag) else $display("%0t a2 failed", $time);
  a3: assert property (@clk !valid |-> !flag) else $display("%0t a3 failed", $time);
  a4: assert property (@clk r >= 8'd10 && r <= 8'd20) else $display("%0t a4 failed", $time);
  a5: assert property (@clk ##1 idx == $past(idx)) else $display("%0t a5 failed", $time);
  a6: assert property (@clk ##1 t != $past(t)) else $display("%0t a6 failed", $time);
  c1: cover property (@clk reset ##1 1) $display("%0t after reset flag=%0d", $time, flag);
  c2: cover property (@clk !valid) $display("%0t invalid flag=%0d", $time, flag);
  initial a_k: assert #0 (k == 4'd9) $display("%0t k=%0d", $time, k);
  // t is assigned, so it is inactive and toggles (17.7.1 example).
  always @(clk) t <= !t;
  always @(clk) begin
    w_seen <= w;
    w_changes <= w_changes + ((w != w_seen) ? 4'd1 : 4'd0);
  end
  final $display("explicit w redrawn=%0d", w_changes > 4'd4);
endchecker
checker implicit_model(event clk);
  rand bit [15:0] v;
  bit [15:0] v_seen = 16'd0;
  bit [3:0] v_changes = 4'd0;
  always @(clk) begin
    v_seen <= v;
    v_changes <= v_changes + ((v != v_seen) ? 4'd1 : 4'd0);
  end
  final $display("implicit v redrawn=%0d", v_changes > 4'd4);
endchecker
module tb;
  bit clk = 1'b0, valid = 1'b1, reset = 1'b1;
  observer_model u(valid, reset, posedge clk);
  implicit_model v(posedge clk);
  always #5 clk = ~clk;
  initial begin
    #22 reset = 1'b0;
    #40 reset = 1'b1;
    #8 valid = 1'b0;
    #20 valid = 1'b1;
    #10 $finish;
  end
endmodule
