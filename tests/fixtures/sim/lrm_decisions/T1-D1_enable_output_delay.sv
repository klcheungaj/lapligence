// Decision T1-D1: a delayed enable gate with an unknown enable drives L or H,
// not X, and its delay follows the new combined output alone: 0 falls, 1
// rises, Z turns off, and X, L and H take the smallest delay, also from L to
// H. Both halves publish together, so the net never shows a value between
// them.
//
// IEEE 1800-2009 28.6 (SystemVerilog-1800-2009.txt L47216-47217): "the first
// delay shall determine the output rise delay, the second delay shall
// determine the output fall delay, and the third delay shall determine the
// delay of transitions to z, and the smallest of the three delays shall
// determine the delay of transitions to x." L47224: "The symbol L shall
// represent a result that has a value 0 or z." The text does not say which
// delay a transition to L or H takes; this case treats L and H like x.
`timescale 1ns/100ps
module tb;
  reg d, e;
  wire w1, w2, w3, w4, w5, w6, w7, w8;
  bufif1 #(5,2,7) g1 (w1, d, e);
  notif1 #(5,2,7) g2 (w2, d, e);
  bufif1 #(2,5,7) g3 (w3, d, e);
  bufif1 #(7,5,2) g4 (w4, d, e);
  bufif1 #1 g5 (w5, d, e);   pulldown (w5);
  bufif0 #1 g6 (w6, d, e);   pullup (w6);
  bufif1 (strong0, weak1) #1 g7 (w7, d, e);  pulldown (w7);
  notif0 (pull0, pull1) #1 g8 (w8, d, e);    pulldown (w8);
  string last;
  always begin : sample
    string now;
    // Halfway between time steps: an update and a read of it never race.
    #0.5;
    now = $sformatf("%v %v %v %v %v %v %v %v", w1, w2, w3, w4, w5, w6, w7, w8);
    if (now != last) $display("%0d %s", $time, now);
    last = now;
    #0.5;
  end
  initial begin
    d = 0; e = 1'bx;            // x to L
    #20 d = 1;                  // L to H
    #20 d = 0;                  // H to L
    #20 e = 0;                  // L to z
    #20 d = 1; e = 1'bx;        // z to H
    #20 e = 0;                  // H to z
    #20 d = 0; e = 1'bx;        // z to L
    #20 e = 1;                  // L to 0
    #20 e = 1'bx;               // 0 to L
    #20 d = 1; e = 1;           // L to 1
    #20 e = 1'bx;               // 1 to H
    #20 d = 0; e = 1;           // H to 0
    #20 d = 1'bx;               // 0 to x
    #20 d = 0; e = 1'bx;        // x to L
    #20 d = 1'bx;               // L to x
    #20 d = 1;                  // x to H
    #20 d = 1'bx;               // H to x
    #20 d = 1; e = 1;           // pending 1 replaced by the unknown output
    #1 e = 1'bx;
    #1 d = 0;
    #20 e = 0;                  // pending turn-off replaced by a fall
    #1 e = 1;
    #20 $finish;
  end
endmodule
