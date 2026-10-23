// ADV-005: gates driving switch nodes and switch nodes driving gate and
// switch controls (IEEE 1800-2009 28.12 combined signals, 28.8), and undriven
// switch networks, which stay high impedance.
`timescale 1ns/1ns
module tb;
  wire a, b, c, d, e;
  reg i, en;
  not (pull0, strong1) g1(a, i);
  tran t1(a, b);
  bufif1 (strong0, weak1) g2(c, i, en);
  rtran t2(c, d);
  tranif1 t3(d, e, b);
  wire o; and g3(o, e, 1'b1);
  wire ua, ub, uc, ud; reg uen;
  tran t4(ua, ub);
  tranif1 t5(uc, ud, uen);
  initial begin
    i = 0; en = 1; #1 $display("1 %v %v %v %v %v %b | %v %v %v %v %b%b", a, b, c, d, e, o, ua, ub, uc, ud, ua, uc);
    i = 1; uen = 1; #1 $display("2 %v %v %v %v %v %b | %v %v %v %v", a, b, c, d, e, o, ua, ub, uc, ud);
    en = 1'bx; #1 $display("3 %v %v %v %v %v %b", a, b, c, d, e, o);
    i = 0; #1 $display("4 %v %v %v %v %v %b", a, b, c, d, e, o);
    $finish;
  end
endmodule
