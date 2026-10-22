`timescale 1ns/1ns
// ADV-008 A01: invocation pulse controls. --pulse-reject and --pulse-error
// set the limits of paths without PATHPULSE$ as truncated percentages of the
// delay of each transition (default 100); PATHPULSE$ takes precedence.
// --pulse-style and --show-cancelled replace every declaration.
module pd(input a, output y);
  buf (y, a);
  specify
    (a => y) = 10;
  endspecify
endmodule

module pp(input a, output y);
  buf (y, a);
  specify
    (a => y) = 10;
    specparam PATHPULSE$a$y = (3, 6);
  endspecify
endmodule

module tb;
  reg a;
  wire y1, y2;
  pd u1(.a(a), .y(y1));
  pp u2(.a(a), .y(y2));
  initial $monitor("%0t y1=%b y2=%b", $time, y1, y2);
  initial begin
    a = 0;
    #30 a = 1; #3 a = 0;     // rise 30, w=3
    #30 a = 1; #4 a = 0;     // rise 63, w=4
    #30 a = 1; #5 a = 0;     // rise 97, w=5
    #30 a = 1; #7 a = 0;     // rise 132, w=7
    #30 a = 1; #8 a = 0;     // rise 169, w=8
    #30 a = 1; #9 a = 0;     // rise 207, w=9
    #30 a = 1; #10 a = 0;    // rise 246, w=10
    #30 $finish;
  end
endmodule
