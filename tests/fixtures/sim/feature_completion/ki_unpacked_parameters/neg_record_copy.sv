module tb;
  typedef struct { int a; string s; } t;
  parameter t P = '{1, "xyz"};
  t v;
  initial begin
    v = P;
    $display("%s", v.s);
  end
endmodule
