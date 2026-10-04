// An event control on an element range wakes only for changes of those
// elements (IEEE 1800-2009 7.4.5, 9.4.2); the declaration initializers run
// before any procedure starts (6.8).
module tb;
  logic [3:0][7:0] w = '0;
  logic [0:3][7:0] a = '0;
  integer i;
  initial begin
    #1 w[0] = 8'h01;
    #1 w[1] = 8'h02;
    #1 w[2] = 8'h03;
    #1 w[3] = 8'h04;
    #1 a[3] = 8'h05;
    #1 a[0] = 8'h06;
    #1 i = 1;
    w[i] = 8'h07;
    #1 $finish(0);
  end
  always @(w[3:2]) $display("high %h", w);
  always @(w[1 -: 2]) $display("low %h", w);
  always @(a[0:1]) $display("ascending %h", a);
endmodule
