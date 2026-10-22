// Decision A8-D9: pulse limits do not change how delays compose. The path
// stage filters its scheduled transitions with the PATHPULSE$ limits and a
// net delay declared on the destination then delays the result, exactly as
// without pulse limits (decision A7-D1).
//
// IEEE 1800-2009 30.7 (SystemVerilog-1800-2009.txt L50262-50264): "Pulse
// width ranges control how to handle a pulse presented at a module path
// output." 30.7.1 (L50331-50334): "The reject limit and error limit may be
// specified for a specific module path." Neither changes the delay.
`timescale 1ns/1ns
module nd(a, y);
  input a;
  output y;
  wire #2 y;
  buf (y, a);
  specify
    (a => y) = 10;
    specparam PATHPULSE$a$y = (2, 5);
  endspecify
endmodule

module np(a, y);
  input a;
  output y;
  wire #2 y;
  buf (y, a);
  specify
    (a => y) = 10;
  endspecify
endmodule

module tb;
  reg a;
  wire y1, y2;
  nd u1(.a(a), .y(y1));
  np u2(.a(a), .y(y2));
  initial $monitor("%0t y1=%b y2=%b", $time, y1, y2);
  initial begin
    a = 0;                   // 10 + 2 -> 12
    #100 a = 1;              // 112
    #50 a = 0;               // 162
    #50 a = 1; #4 a = 0;     // w=4: y1 x 212..216; y2 rejected
    #100 $finish;
  end
endmodule
