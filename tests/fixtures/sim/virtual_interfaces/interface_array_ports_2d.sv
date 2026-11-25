// Two-dimensional arrays of interface instances through ports (25.3).
interface ifc;
  int n;
endinterface

module pair(ifc p[2]);
  virtual ifc v[2];
  int a, b;
  initial begin
    #1 v = p;
    a = v[0].n;
    b = p[1].n;
  end
endmodule

module grid(ifc p[2][2]);
  virtual ifc v[2][2];
  initial begin
    #1 v = '{'{p[0][0], p[0][1]}, '{p[1][0], p[1][1]}};
    $display("%0d %0d %0d %0d", v[0][0].n, v[0][1].n, v[1][0].n, v[1][1].n);
  end
endmodule

module tb;
  ifc i[2][1:0]();
  grid g(i);
  pair q[2](i);
  initial begin
    i[0][0].n = 1;
    i[0][1].n = 2;
    i[1][0].n = 3;
    i[1][1].n = 4;
    #2 $display("%0d %0d %0d %0d", q[0].a, q[0].b, q[1].a, q[1].b);
    $finish(0);
  end
endmodule
