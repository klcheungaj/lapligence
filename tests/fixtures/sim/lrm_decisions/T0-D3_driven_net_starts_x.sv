// Decision T0-D3: a net with drivers assumes the output of its drivers, which
// is X until they first evaluate at time 0. Its driven bits therefore start
// at X; bits no driver writes keep the default z. A driver whose first value
// is X makes no time-0 change, so an `always @(w)` that is already waiting
// (T0-D2) does not wake for it; a driver whose first value is 0 or 1 is a
// change. The value sampled at time 0 is the start value (T0-D5).
//
// IEEE 1800-2009 6.7 (SystemVerilog-1800-2009.txt L5199-5200):
//   "The default initialization value for a net shall be the value z. Nets
//   with drivers shall assume the output value of their drivers."
// IEEE 1800-2009 4.9.1 (L3509-3511):
//   "A continuous assignment process is also evaluated at time 0 in order to
//   propagate constant values. This includes implicit continuous assignments
//   inferred from port connections (see 4.9.6)."
// IEEE 1800-2009 16.9.3 (L22672-22673):
//   "The value of an expression sampled in the Preponed region corresponding
//   to time 0 is the result of evaluating the expression using the initial
//   values of the variables comprising the expression."
module tb;
  logic [7:0] r;
  logic [3:0] r4;
  wire [7:0] wx;
  wire [7:0] wk;
  wire [7:0] wp;
  wire [7:0] wu;
  bit ev_wx, ev_wk, ev_wp;
  assign wx = r;
  assign wk = 8'h12;
  assign wp[3:0] = r4;
  always @(wx) ev_wx <= 1'b1;
  always @(wk) ev_wk <= 1'b1;
  always @(wp) ev_wp <= 1'b1;
  initial begin
    $display("t0 sampled wx=%h wp=%h wu=%h", $sampled(wx), $sampled(wp), $sampled(wu));
    #1 $display("t1 wx=%h wk=%h wp=%h wu=%h", wx, wk, wp, wu);
    $display("time-0 events: x-valued=%0d constant=%0d partial=%0d", ev_wx, ev_wk, ev_wp);
    r = 8'h0f;
    r4 = 4'h3;
    #1 $display("t2 wx=%h wp=%h events: x-valued=%0d partial=%0d", wx, wp, ev_wx, ev_wp);
    $finish;
  end
endmodule
