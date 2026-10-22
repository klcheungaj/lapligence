// Decision A8-D7: a pulsestyle_ondetect declaration applies on its own: a
// pulse filtered to x by the error limit turns x when it is detected (when
// its trailing transition is scheduled), with or without showcancelled.
//
// IEEE 1800-2009 30.7.4.1 (SystemVerilog-1800-2009.txt L50417-50423): "When
// an output pulse is to be filtered to X, the leading edge of the pulse
// becomes a transition to X, and the trailing edge becomes a transition from
// X. The times of transition of the edges do not change. Just like on-event,
// the on-detect method of pulse filtering changes the leading edge of the
// pulse into a transition to X and the trailing edge to a transition from X,
// but the time of the leading edge is changed to occur immediately upon
// detection of the pulse."
`timescale 1ns/1ns
module od(input a, output y);
  buf (y, a);
  specify
    pulsestyle_ondetect y;
    (a => y) = (5, 5);
    specparam PATHPULSE$a$y = (2, 4);
  endspecify
endmodule

module oe(input a, output y);
  buf (y, a);
  specify
    (a => y) = (5, 5);
    specparam PATHPULSE$a$y = (2, 4);
  endspecify
endmodule

module tb;
  reg a;
  wire y1, y2;
  od u1(.a(a), .y(y1));
  oe u2(.a(a), .y(y2));
  initial $monitor("%0t y1=%b y2=%b", $time, y1, y2);
  initial begin
    a = 0;
    #100 a = 1;              // rise due 105
    #3 a = 0;                // fall due 108: w=3, detected at 103
    #100 $finish;
  end
endmodule
