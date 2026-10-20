`timescale 1ns/1ns
// ADV-007 A01: each path delay form (1, 2, 3, 6 and 12 values) on every
// 0/1/x/z transition of its destination. Each change is 100 units apart, so
// every transition matures before the next one starts.
module p1(input a, output y);
  assign y = a;
  specify
    (a => y) = 4;
  endspecify
endmodule

module p2(input a, output y);
  assign y = a;
  specify
    (a => y) = (3, 5);
  endspecify
endmodule

module p3(input a, output y);
  assign y = a;
  specify
    (a => y) = (1, 2, 3);
  endspecify
endmodule

module p6(input a, output y);
  assign y = a;
  specify
    (a => y) = (1, 2, 3, 4, 5, 6);
  endspecify
endmodule

module p12(input a, output y);
  assign y = a;
  specify
    (a *> y) = (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12);
  endspecify
endmodule

module tb;
  reg a;
  wire y1, y2, y3, y6, y12;
  p1 u1(.a(a), .y(y1));
  p2 u2(.a(a), .y(y2));
  p3 u3(.a(a), .y(y3));
  p6 u6(.a(a), .y(y6));
  p12 u12(.a(a), .y(y12));
  initial $monitor("%0t a=%b y1=%b y2=%b y3=%b y6=%b y12=%b", $time, a, y1, y2, y3, y6, y12);
  initial begin
    a = 1'b0;
    #100 a = 1'b1;
    #100 a = 1'b0;
    #100 a = 1'bz;
    #100 a = 1'b1;
    #100 a = 1'bz;
    #100 a = 1'b0;
    #100 a = 1'bx;
    #100 a = 1'b1;
    #100 a = 1'bx;
    #100 a = 1'b0;
    #100 a = 1'bx;
    #100 a = 1'bz;
    #100 a = 1'bx;
    #100 $finish(0);
  end
endmodule
