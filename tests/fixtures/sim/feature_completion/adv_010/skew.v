// $timeskew and $fullskew with the event_based and remain_active flags
// (module name suffix: event_based, remain_active). Each checked module
// counts its notifier changes and keeps the time of the last one (0: none).
`timescale 1ns/1ns
module tsk00(input CP, input CPN, input MODE);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $timeskew(posedge CP &&& MODE, negedge CPN, 50, n, 0, 0);
  endspecify
endmodule
module tsk01(input CP, input CPN, input MODE);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $timeskew(posedge CP &&& MODE, negedge CPN, 50, n, 0, 1);
  endspecify
endmodule
module tsk10(input CP, input CPN, input MODE);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $timeskew(posedge CP &&& MODE, negedge CPN, 50, n, 1, 0);
  endspecify
endmodule
module tsk11(input CP, input CPN, input MODE);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $timeskew(posedge CP &&& MODE, negedge CPN, 50, n, 1, 1);
  endspecify
endmodule
module fsk00(input CP, input CPN, input MODE);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $fullskew(posedge CP &&& MODE, negedge CPN, 50, 70, n, 0, 0);
  endspecify
endmodule
module fsk01(input CP, input CPN, input MODE);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $fullskew(posedge CP &&& MODE, negedge CPN, 50, 70, n, 0, 1);
  endspecify
endmodule
module fsk10(input CP, input CPN, input MODE);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $fullskew(posedge CP &&& MODE, negedge CPN, 50, 70, n, 1, 0);
  endspecify
endmodule
module fsk11(input CP, input CPN, input MODE);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $fullskew(posedge CP &&& MODE, negedge CPN, 50, 70, n, 1, 1);
  endspecify
endmodule
module tsz0(input CP, input CPN, input MODE);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $timeskew(posedge CP &&& MODE, negedge CPN, 0, n);
  endspecify
endmodule
module tsz1(input CP, input CPN, input MODE);
  reg n;
  integer count = 0;
  time last = 0;
  initial forever @(n) begin count = count + 1; last = $time; end
  specify
    $timeskew(posedge CP &&& MODE, negedge CPN, 0, n, 1);
  endspecify
endmodule
module tb;
  // Figure 31-1: A=100, B=150, C=160, D=170, E=180, F=200 (MODE 0), G..J=210..240.
  reg w1_MODE, w1_CP, w1_CPN;
  initial begin
    w1_MODE = 1'b1;
    w1_CP = 1'b0;
    w1_CPN = 1'b1;
    #100 w1_CP = 1'b1;
    #20 w1_CP = 1'b0;
    #40 w1_CPN = 1'b0;
    #5 w1_CPN = 1'b1;
    #5 w1_CPN = 1'b0;
    #5 w1_CPN = 1'b1;
    #5 w1_CPN = 1'b0;
    #5 w1_CPN = 1'b1;
    #5 w1_MODE = 1'b0;
    #10 w1_CP = 1'b1;
    #10 w1_CPN = 1'b0;
    #5 w1_CPN = 1'b1;
    #5 w1_CPN = 1'b0;
    #5 w1_CPN = 1'b1;
    #5 w1_CPN = 1'b0;
    #5 w1_CPN = 1'b1;
    #5 w1_CPN = 1'b0;
  end
  tsk00 i1a(w1_CP, w1_CPN, w1_MODE);
  tsk01 i1b(w1_CP, w1_CPN, w1_MODE);
  tsk10 i1c(w1_CP, w1_CPN, w1_MODE);
  tsk11 i1d(w1_CP, w1_CPN, w1_MODE);
  // Figure 31-2: A=100, F=120 (MODE 0), B=150, CPN falls at 170 and 180.
  reg w2_MODE, w2_CP, w2_CPN;
  initial begin
    w2_MODE = 1'b1;
    w2_CP = 1'b0;
    w2_CPN = 1'b1;
    #100 w2_CP = 1'b1;
    #10 w2_MODE = 1'b0;
    #5 w2_CP = 1'b0;
    #5 w2_CP = 1'b1;
    #50 w2_CPN = 1'b0;
    #5 w2_CPN = 1'b1;
    #5 w2_CPN = 1'b0;
  end
  tsk00 i2a(w2_CP, w2_CPN, w2_MODE);
  tsk01 i2b(w2_CP, w2_CPN, w2_MODE);
  tsk10 i2c(w2_CP, w2_CPN, w2_MODE);
  tsk11 i2d(w2_CP, w2_CPN, w2_MODE);
  // $timeskew boundaries.
  reg b1_MODE, b1_CP, b1_CPN;
  initial begin
    b1_MODE = 1'b1;
    b1_CP = 1'b0;
    b1_CPN = 1'b1;
    #100 b1_CP = 1'b1;
    #50 b1_CPN = 1'b0;
  end
  tsk00 b1(b1_CP, b1_CPN, b1_MODE);
  reg b2_MODE, b2_CP, b2_CPN;
  initial begin
    b2_MODE = 1'b1;
    b2_CP = 1'b0;
    b2_CPN = 1'b1;
    #100 b2_CP = 1'b1;
    #51 b2_CPN = 1'b0;
  end
  tsk00 b2(b2_CP, b2_CPN, b2_MODE);
  reg b3_MODE, b3_CP, b3_CPN;
  initial begin
    b3_MODE = 1'b1;
    b3_CP = 1'b0;
    b3_CPN = 1'b1;
    #100 b3_CP = 1'b1;
    #25 b3_CP = 1'b0;
    #25 b3_CP = 1'b1;
    #40 b3_CPN = 1'b0;
  end
  tsk00 b3(b3_CP, b3_CPN, b3_MODE);
  reg b4_MODE, b4_CP, b4_CPN;
  initial begin
    b4_MODE = 1'b1;
    b4_CP = 1'b0;
    b4_CPN = 1'b1;
    #100 b4_CP = 1'b1;
    #20 b4_CP = 1'b0;
    #10 b4_CP = 1'b1;
    #45 b4_CPN = 1'b0;
  end
  tsk00 b4(b4_CP, b4_CPN, b4_MODE);
  reg b5_MODE, b5_CP, b5_CPN;
  initial begin
    b5_MODE = 1'b1;
    b5_CP = 1'b0;
    b5_CPN = 1'b1;
    #100 b5_CP = 1'b1;
    b5_CPN = 1'b0;
  end
  tsz0 b5(b5_CP, b5_CPN, b5_MODE);
  reg b6_MODE, b6_CP, b6_CPN;
  initial begin
    b6_MODE = 1'b1;
    b6_CP = 1'b0;
    b6_CPN = 1'b1;
    #100 b6_CP = 1'b1;
    #1 b6_CPN = 1'b0;
  end
  tsz1 b6(b6_CP, b6_CPN, b6_MODE);
  reg b7_MODE, b7_CP, b7_CPN;
  initial begin
    b7_MODE = 1'b1;
    b7_CP = 1'b0;
    b7_CPN = 1'b1;
    #100 b7_CP = 1'b1;
    #1 b7_CPN = 1'b0;
  end
  tsz0 b7(b7_CP, b7_CPN, b7_MODE);
  // $fullskew: CP 100, CPN 130 (within 50), CPN 200, CP 300 (100 > 70), CPN 330.
  reg f1_MODE, f1_CP, f1_CPN;
  initial begin
    f1_MODE = 1'b1;
    f1_CP = 1'b0;
    f1_CPN = 1'b1;
    #100 f1_CP = 1'b1;
    #10 f1_CP = 1'b0;
    #20 f1_CPN = 1'b0;
    #10 f1_CPN = 1'b1;
    #60 f1_CPN = 1'b0;
    #10 f1_CPN = 1'b1;
    #90 f1_CP = 1'b1;
    #10 f1_CP = 1'b0;
    #20 f1_CPN = 1'b0;
    #10 f1_CPN = 1'b1;
  end
  fsk00 f1a(f1_CP, f1_CPN, f1_MODE);
  fsk01 f1b(f1_CP, f1_CPN, f1_MODE);
  fsk10 f1c(f1_CP, f1_CPN, f1_MODE);
  fsk11 f1d(f1_CP, f1_CPN, f1_MODE);
  // $fullskew: CP 100, CP 120 with MODE 0, CPN 170.
  reg f2_MODE, f2_CP, f2_CPN;
  initial begin
    f2_MODE = 1'b1;
    f2_CP = 1'b0;
    f2_CPN = 1'b1;
    #100 f2_CP = 1'b1;
    #5 f2_MODE = 1'b0;
    #5 f2_CP = 1'b0;
    #10 f2_CP = 1'b1;
    #50 f2_CPN = 1'b0;
  end
  fsk00 f2a(f2_CP, f2_CPN, f2_MODE);
  fsk01 f2b(f2_CP, f2_CPN, f2_MODE);
  fsk10 f2c(f2_CP, f2_CPN, f2_MODE);
  fsk11 f2d(f2_CP, f2_CPN, f2_MODE);
  // $fullskew boundaries.
  reg f3_MODE, f3_CP, f3_CPN;
  initial begin
    f3_MODE = 1'b1;
    f3_CP = 1'b0;
    f3_CPN = 1'b1;
    #200 f3_CPN = 1'b0;
    #70 f3_CP = 1'b1;
  end
  fsk00 f3(f3_CP, f3_CPN, f3_MODE);
  reg f4_MODE, f4_CP, f4_CPN;
  initial begin
    f4_MODE = 1'b1;
    f4_CP = 1'b0;
    f4_CPN = 1'b1;
    #100 f4_CP = 1'b1;
    f4_CPN = 1'b0;
  end
  fsk00 f4(f4_CP, f4_CPN, f4_MODE);
  initial begin
    #400;
    $display("i1a %0d@%0d i1b %0d@%0d i1c %0d@%0d i1d %0d@%0d",
             i1a.count, i1a.last, i1b.count, i1b.last, i1c.count, i1c.last, i1d.count, i1d.last);
    $display("i2a %0d@%0d i2b %0d@%0d i2c %0d@%0d i2d %0d@%0d",
             i2a.count, i2a.last, i2b.count, i2b.last, i2c.count, i2c.last, i2d.count, i2d.last);
    $display("b1 %0d@%0d b2 %0d@%0d b3 %0d@%0d b4 %0d@%0d b5 %0d@%0d b6 %0d@%0d b7 %0d@%0d",
             b1.count, b1.last, b2.count, b2.last, b3.count, b3.last, b4.count, b4.last, b5.count, b5.last, b6.count, b6.last, b7.count, b7.last);
    $display("f1a %0d@%0d f1b %0d@%0d f1c %0d@%0d f1d %0d@%0d",
             f1a.count, f1a.last, f1b.count, f1b.last, f1c.count, f1c.last, f1d.count, f1d.last);
    $display("f2a %0d@%0d f2b %0d@%0d f2c %0d@%0d f2d %0d@%0d f3 %0d@%0d f4 %0d@%0d",
             f2a.count, f2a.last, f2b.count, f2b.last, f2c.count, f2c.last, f2d.count, f2d.last, f3.count, f3.last, f4.count, f4.last);
    $finish;
  end
endmodule
