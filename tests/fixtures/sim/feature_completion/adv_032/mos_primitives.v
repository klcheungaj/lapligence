module tb;
  reg d, c;
  wire o1, o2, o3, o4, o5, o6;
  nmos n1 (o1, d, c);
  pmos p1 (o2, d, c);
  rnmos rn1 (o3, d, c);
  rpmos rp1 (o4, d, c);
  cmos cm1 (o5, d, c, c);
  rcmos rcm1 (o6, d, c, c);
  initial begin
    d = 1'b1;
    c = 1'b1;
    #1 $display("%b%b%b%b%b%b", o1, o2, o3, o4, o5, o6);
    $finish;
  end
endmodule
