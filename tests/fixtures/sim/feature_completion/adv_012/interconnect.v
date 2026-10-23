// ADV-012 interconnect: PORT, INTERCONNECT and NETDELAY delays (IEEE
// 1800-2009 32.4.4, Table 32-3) to load ports: one source, several sources
// combined by each INTERCONNECT_MIPD policy, a NETDELAY to a port, to every
// load port of a net and to the load ports of one bit of a vector net, and
// an INTERCONNECT whose source does not exist. Oracles in readme.md.
`timescale 1ns/1ps

// Cell library without delays: only interconnect delays apply.
module BUF (input a, output y);
  buf b (y, a);
endmodule

module BUF2 (input [1:0] a, output [1:0] y);
  buf b0 (y[0], a[0]);
  buf b1 (y[1], a[1]);
endmodule

module DRV (input a, input en, output y);
  bufif1 b (y, a, en);
endmodule

// A net with two drivers and one load.
module MS (input a, input en1, input en2, output y);
  wire m;
  DRV d1 (a, en1, m);
  DRV d2 (a, en2, m);
  BUF lm (m, y);
endmodule

module tb;
  reg a = 1'b0, en1 = 1'b1, en2 = 1'b0;
  wire n1, n2, n3, n4, n6, o1, o2, o3, o4a, o4b, o5b, o6;
  wire [1:0] n5, o5;
  wire [3:0] ms;
  BUF s1 (a, n1);
  BUF l1 (n1, o1);
  BUF s2 (a, n2);
  BUF l2 (n2, o2);
  MS ms0 (a, en1, en2, ms[0]);
  MS ms1 (a, en1, en2, ms[1]);
  MS ms2 (a, en1, en2, ms[2]);
  MS ms3 (a, en1, en2, ms[3]);
  BUF s3 (a, n3);
  BUF l3 (n3, o3);
  BUF s4 (a, n4);
  BUF l4a (n4, o4a);
  BUF l4b (n4, o4b);
  BUF2 s5 ({a, a}, n5);
  BUF2 l5 (n5, o5);
  BUF l5b (n5[1], o5b);
  BUF s6 (a, n6);
  BUF l6 (n6, o6);
  initial begin
    $sdf_annotate("interconnect.sdf");
    $sdf_annotate("netdelay.sdf");
    $sdf_annotate("ms.sdf", ms0);
    $sdf_annotate("ms.sdf", ms1, "min.cfg");
    $sdf_annotate("ms.sdf", ms2, "average.cfg");
    $sdf_annotate("ms.sdf", ms3, "last.cfg");
    $monitor("%0t a=%b en=%b%b o1=%b o2=%b ms=%b o3=%b o4=%b%b o5=%b o5b=%b o6=%b",
             $realtime, a, en1, en2, o1, o2, ms, o3, o4a, o4b, o5, o5b, o6);
    #20 a = 1'b1;
    #20 a = 1'b0;
    #20 begin
      en1 = 1'b0;
      en2 = 1'b1;
    end
    #20 a = 1'b1;
    #20 a = 1'b0;
    #20 $finish;
  end
endmodule
