// Decision A10-D10: a delayed signal follows its signal with transport
// delays, the rise delay for 01, 0x and x1 transitions and the fall delay
// for the others; a transition that lands before a pending one cancels it,
// so pulses survive unless they are reordered.
//
// IEEE 1800-2009 31.9.1 (SystemVerilog-1800-2009.txt L52381-52384): "delayed
// copies of the data and reference signals are generated in the timing
// checks, and these are used internally for timing check evaluation at run
// time. The setup and hold times used internally are adjusted to shift the
// violation window and make it overlap the reference signal."
`timescale 1ns/1ns
// Rising data delayed by 6 (1 - (-5)), falling data by 2 (1 - (-1)).
module pg(input C, input D); reg n; wire dC, dD;
  specify
    $setuphold(posedge C, posedge D, 8, -5, n, , , dC, dD);
    $setuphold(posedge C, negedge D, 8, -1, n, , , dC, dD);
  endspecify
endmodule
module tb;
  reg c, d;
  pg u(c, d);
  always @(u.dD) if ($time > 0) $display("%0t dD=%b", $time, u.dD);
  initial begin
    c = 0; d = 0;
    #100 d = 1;           // lands at 106 ...
    #1 d = 0;             // ... cancelled by the fall landing at 103
    #9 d = 1;             // 110: lands at 116
    #1 d = 1'bx;          // 111: 1x is a fall: lands at 113, cancels 116
    #9 d = 0;             // 120: x0 lands at 122
    #10 d = 1;            // 130: lands at 136
    #1 d = 0;             // 131: lands at 133, cancels 136
    #2 d = 1;             // 133: lands at 139
    #20 $finish;
  end
endmodule
