// Arrays of interface instances through interface-array ports (25.3, 25.9,
// 23.3.3.5, 7.6): elements, slices, whole arrays as virtual interface arrays
// matched left to left, nested modules, modports and arrays of instances.
`timescale 1ns/1ns
interface ifc;
  int n;
  modport mp(input n);
endinterface

module whole(ifc p[2]);
  parameter int AT = 1;
  virtual ifc v[2];
  function void show(virtual ifc x[2]);
    $display("%m %0d %0d", x[0].n, x[1].n);
  endfunction
  initial begin
    #AT show(p);
    v = p;
    show(v);
    v = '{p[1], p[0]};
    show(v);
    v[0] = p[0];
    show(v);
    v[1].n = 20;
    $display("%0d", p[0].n);
    v[1].n = 1;
  end
endmodule

module descending(ifc p[1:0]);
  parameter int AT = 2;
  virtual ifc v[2];
  virtual ifc w[1:0];
  initial begin
    #AT v = p;
    w = p;
    $display("%m %0d %0d %0d %0d", v[0].n, v[1].n, w[0].n, w[1].n);
    $display("%m %0d %0d", p[0].n, p[1].n);
  end
endmodule

module part(ifc p[3]);
  virtual ifc v[2];
  initial begin
    #3 v = p[1:2];
    $display("%m %0d %0d", v[0].n, v[1].n);
  end
endmodule

module single(ifc p);
  parameter int AT = 4;
  virtual ifc v;
  initial begin
    #AT v = p;
    if (AT != 13) $display("%m %0d %0d", p.n, v.n);
  end
endmodule

module view(ifc.mp p[2]);
  initial #7 $display("%m %0d %0d", p[0].n, p[1].n);
endmodule

module middle(ifc p[2]);
  whole #(.AT(11)) s(p);
  single #(.AT(5)) d(p[1]);
  genvar g;
  for (g = 0; g < 2; g++) begin : gg
    single #(.AT(8 + g)) k(p[g]);
  end
endmodule

module tb;
  ifc i[2]();
  ifc j[3]();
  ifc k[1:0]();
  virtual ifc v[2];
  virtual ifc w[1:0];
  whole a(i);
  descending b(i);
  part c(j);
  single #(.AT(6)) e(j[2]);
  view f(i);
  whole #(.AT(12)) h(j[1:2]);
  middle m(i);
  single #(.AT(13)) l[2](k);
  descending #(.AT(14)) o(k);
  initial begin
    i[0].n = 1;
    i[1].n = 2;
    j[0].n = 3;
    j[1].n = 4;
    j[2].n = 5;
    k[0].n = 6;
    k[1].n = 7;
    #10 v = k;
    w = k;
    $display("%0d %0d %0d %0d", v[0].n, v[1].n, w[0].n, w[1].n);
    v = i;
    w = i;
    $display("%0d %0d %0d %0d", v[0].n, v[1].n, w[0].n, w[1].n);
    #4 $display("%0d %0d", l[0].v.n, l[1].v.n);
    #10 $finish(0);
  end
endmodule

