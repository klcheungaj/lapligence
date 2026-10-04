// RTL-012: an unknown enable gives L/H (IEEE 1364-2001 7.4, 7.10.2-7.10.3),
// which a weaker same-value driver resolves to a known value. Hand-derived.
module tb;
  reg d, e;
  wire w0, w1, w2, w3, pd, pu, l0;
  bufif1 (strong0, weak1) b0(w0, d, e);
  bufif0 b1(w1, d, e);
  notif1 (pull0, strong1) b2(w2, d, e);
  and (highz0, strong1) a3(w3, d, e);
  bufif1 b4(pd, d, e);
  pulldown (pd);
  bufif1 b5(pu, d, e);
  pullup (pu);
  bufif0 b6(l0, d, e);
  assign (weak0, weak1) l0 = 1'b0;
  wire [1:0] na [0:1];
  bufif1 b7(na[1][0], d, e);
  pulldown (na[1][0]);
  initial begin
    d = 0; e = 1'bx;
    #1 $display("%v %v %v %v | %v %b %v %b %v %b | %v %b", w0, w1, w2, w3, pd, pd, pu, pu, l0, l0, na[1][0], na[1][0]);
    d = 1;
    #1 $display("%v %v %v %v | %v %b %v %b %v %b | %v %b", w0, w1, w2, w3, pd, pd, pu, pu, l0, l0, na[1][0], na[1][0]);
    e = 1;
    #1 $display("%v %v %v %v | %v %b %v %b %v %b | %v %b", w0, w1, w2, w3, pd, pd, pu, pu, l0, l0, na[1][0], na[1][0]);
    d = 1'bz;
    #1 $display("%v %v %v %v | %v %b %v %b %v %b | %v %b", w0, w1, w2, w3, pd, pd, pu, pu, l0, l0, na[1][0], na[1][0]);
    e = 0;
    #1 $display("%v %v %v %v | %v %b %v %b %v %b | %v %b", w0, w1, w2, w3, pd, pd, pu, pu, l0, l0, na[1][0], na[1][0]);
    $finish(0);
  end
endmodule
