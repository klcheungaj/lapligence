// Decision A5-D4: a tranif delay delays the switch's conduction state, not
// the signals through it: the state follows the control with the turn-on
// delay to on, the turn-off delay to off and the smaller delay to unknown (x
// or z), a newer control change replacing a pending one; a delayed switch is
// unknown (A5-D1) until its first delay elapses. Nodes are sampled in the
// middle of each time unit and printed when they change.
//
// IEEE 1800-2009 28.8 (SystemVerilog-1800-2009.txt L47365, L47380-47384):
// "The bidirectional pass switches shall not delay signals propagating through
// them." "If the specification contains two delays, the first delay shall
// determine the turn-on delay, the second delay shall determine the turn-off
// delay, and the smaller of the two delays shall apply to output transitions
// to x and z."
`timescale 1ns/100ps
module tb;
  reg c, ra;
  wire a, b;
  assign (strong0, strong1) a = ra;
  tranif1 #(3, 5) s1(a, b, c);
  reg [8*4-1:0] now, last;
  integer i;
  initial begin
    last = 32'd0;
    #0.5;
    for (i = 0; i < 50; i = i + 1) begin
      $sformat(now, "%v", b);
      if (now != last) $display("%0d %0s", i, now);
      last = now;
      #1;
    end
  end
  initial begin
    ra = 1; c = 0;
    #10 c = 1;
    #10 ra = 0;
    #10 c = 1'bx;
    #10 c = 1;
    #1 c = 0;
    #8 $finish;
  end
endmodule
