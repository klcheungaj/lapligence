// ADV-005: switches across module ports and a net delay on a switch node.
// IEEE 1800-2009 4.9.6: "Bidirectional connections are analogous to an
// always-enabled tran connection between the two nets, but without any
// strength reduction." Decision A5-D5: a net delay on a member net delays
// only that net's visible value (6.7.1 net delay); the network sees the
// net's drivers.
`timescale 1ns/1ns
module sw(inout p, inout q, input en);
  tranif1 t(p, q, en);
endmodule
module pass(inout x, inout y);
  tran t(x, y);
endmodule
module tb;
  wire a, b, c;
  reg ra, en;
  supply1 vdd;
  assign (strong0, strong1) a = ra;
  sw s1(a, b, en);
  pass p1(b, c);
  wire d;
  pass p2(vdd, d);
  integer n = 0;
  always @(c) n = n + 1;
  wire y; assign y = ~c;
  // Net delay on the middle net of a tran chain.
  wire da; wire #3 db; wire dc;
  reg rd;
  assign (strong0, strong1) da = rd;
  tran t1(da, db);
  tran t2(db, dc);
  initial begin
    ra = 1; en = 1; rd = 0; #1
    $display("1 %v %v %v %v %b %v", a, b, c, d, y, s1.q);
    en = 0; #1
    $display("2 %v %v %v %v %b %v", a, b, c, d, y, s1.q);
    en = 1; ra = 0; #1
    $display("3 %v %v %v %v %b %v n=%0d", a, b, c, d, y, s1.q, n);
    #7 rd = 1;
    #1 $display("11 %v %v %v", da, db, dc);
    #2 $display("13 %v %v %v", da, db, dc);
    #1 $display("14 %v %v %v", da, db, dc);
    $finish;
  end
endmodule
