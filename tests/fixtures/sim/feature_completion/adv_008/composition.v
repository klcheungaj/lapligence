`timescale 1ns/1ns
// ADV-008 A01: pulse filtering composed with distributed delays, a
// destination net delay and force/release. An inertial gate inside the
// module filters its own pulses first; the path stage filters the scheduled
// path transitions with the PATHPULSE$ limits, at max(arrival, input + path
// delay); a net delay on the destination applies afterwards. A force drops
// every pending path transition; after the release the destination takes
// the value of its drivers.
module dg(input a, output y);
  buf #3 (y, a);
  specify
    (a => y) = 10;
    specparam PATHPULSE$a$y = (2, 5);
  endspecify
endmodule

module dl(input a, output y);
  buf #15 (y, a);
  specify
    (a => y) = 10;
    specparam PATHPULSE$a$y = (2, 5);
  endspecify
endmodule

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

module tr(input a, output y);
  buf (y, a);
  specify
    (a => y) = 10;
    specparam PATHPULSE$ = 0;
  endspecify
endmodule

module tb;
  reg a, b;
  wire y1, y2, y3, y4;
  dg u1(.a(a), .y(y1));
  dl u2(.a(a), .y(y2));
  nd u3(.a(a), .y(y3));
  tr u4(.a(b), .y(y4));
  initial $monitor("%0t y1=%b y2=%b y3=%b y4=%b", $time, y1, y2, y3, y4);
  initial begin
    a = 0; b = 0;
    #100 a = 1; #2 a = 0;    // w=2: the gate (#3) filters it
    #98 a = 1; #4 a = 0;     // w=4: y1 x 210..214; y3 x 212..216
    #96 a = 1; #5 a = 0;     // w=5: y1 1 310..315; y3 1 312..317
    #95 a = 1; #16 a = 0;    // w=16: y2 follows the gate, 415..431
    #84 b = 1; #4 b = 0; #4 b = 1;   // y4 due 510, 514, 518
    #2 force u4.y = 1'b1;    // 510: pending 514 and 518 dropped
    #20 release u4.y;        // 530: back to b = 1
    #10 b = 0;               // 540: y4 0 at 550
  end
  initial #600 $finish;
endmodule
