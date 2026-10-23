// ADV-005 decision A5-D4: a tranif delay delays the switch state, not the
// signals (IEEE 1800-2009 28.8: "the first delay shall determine the turn-on
// delay, the second delay shall determine the turn-off delay, and the smaller
// of the two delays shall apply to output transitions to x and z"). A delayed
// switch is unknown until its first delay elapses. The nodes are sampled in
// the middle of every time unit and printed when their value or strength
// changed.
`timescale 1ns/100ps
module tb;
  reg c, ra;
  wire a, b, a2, b2, a3, b3, a4, b4;
  assign (strong0, strong1) a = ra;
  assign (strong0, strong1) a2 = ra;
  assign (strong0, strong1) a3 = ra;
  assign (strong0, strong1) a4 = ra;
  tranif1 #(3, 5) s1(a, b, c);
  tranif1 #4 s2(a2, b2, c);
  tranif0 #(2, 6) s3(a3, b3, c);
  rtranif0 #(1:2:3, 4:5:6) s4(a4, b4, c);
  reg [8*16-1:0] now, last;
  integer i;
  initial begin
    last = 128'd0;
    #0.5;
    for (i = 0; i < 80; i = i + 1) begin
      $sformat(now, "%v %v %v %v", b, b2, b3, b4);
      if (now != last) $display("%0d %0s", i, now);
      last = now;
      #1;
    end
  end
  initial begin
    ra = 1; c = 0;
    #10 c = 1;
    #10 ra = 0;
    #10 c = 0;
    #10 c = 1'bx;
    #10 c = 1;
    #10 c = 1'bz;
    #10 c = 0;
    #1 c = 1;
    #1 c = 0;
    #20 $finish;
  end
endmodule
