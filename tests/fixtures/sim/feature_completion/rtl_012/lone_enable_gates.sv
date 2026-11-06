// RTL-012: lone delayed enable gates through known outputs, L, H, Z and a force;
// values and times compared with industry-standard output.
`timescale 1ns/1ns
module leaf(input i);
  always @(i) $display("%0t leaf %b %v", $time, i, i);
endmodule
module tb;
  reg d, e;
  wire a, b, c, f;
  bufif1 #(1,2,3) g1 (a, d, e);
  bufif0 (pull0, weak1) #1 g2 (b, d, e);
  notif1 (highz0, strong1) #2 g3 (c, d, e);
  notif0 #(2,1) g4 (f, d, e);
  leaf u(a);
  always @(a) $display("%0t a %b %v", $time, a, a);
  always @(b) $display("%0t b %b %v", $time, b, b);
  always @(c) $display("%0t c %b %v", $time, c, c);
  always @(f) $display("%0t f %b %v", $time, f, f);
  initial begin
    d = 0; e = 0;
    #5 e = 1;
    #5 d = 1;
    #5 e = 1'bx;
    #5 d = 0;
    #5 e = 1;
    #1 d = 1;
    #1 d = 0;
    #5 force a = 1'b1;
    #5 d = 1;
    #5 d = 0;
    #5 release a;
    #5 d = 1'bx;
    #5 e = 0;
    #5 d = 1'bz; e = 1;
    #5 $finish(0);
  end
endmodule
