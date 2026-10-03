// SV2009 13.3, 23.8, 25.7, 26.3; V2001 12.4: package, interface and module
// subroutines keep their owning environment; static state is per declaring
// instance (one shared package), and fixed aggregates cross every boundary.
package pkg;
  typedef logic [7:0] arr_t [0:3];
  typedef struct { logic [7:0] a; logic [7:0] b; } rec_t;
  int calls = 0;
  function automatic arr_t inc(input arr_t x);
    foreach (x[i]) inc[i] = x[i] + 8'd1;
  endfunction
  function automatic rec_t mk(input logic [7:0] v);
    mk.a = v;
    mk.b = ~v;
  endfunction
  function void note();
    calls++;
  endfunction
endpackage

interface ifc;
  import pkg::*;
  arr_t store;
  int puts = 0;
  function automatic void put(input arr_t x);
    store = x;
    puts++;
  endfunction
  function automatic arr_t get();
    return store;
  endfunction
endinterface

module child #(parameter logic [7:0] OFF = 8'd0);
  import pkg::*;
  arr_t mem;
  int ncalls = 0;
  function void setm(input arr_t x);
    foreach (x[i]) mem[i] = x[i] + OFF;
    ncalls++;
  endfunction
  function arr_t getm();
    return mem;
  endfunction
  function void swap_rec(ref rec_t r);
    logic [7:0] t;
    t = r.a;
    r.a = r.b + OFF;
    r.b = t;
  endfunction
endmodule

module tb;
  import pkg::*;
  ifc i0();
  ifc i1();
  child #(.OFF(8'd0)) c0();
  child #(.OFF(8'd100)) c1();
  arr_t a, b;
  rec_t r;
  initial begin
    a = '{8'd1, 8'd2, 8'd3, 8'd4};
    b = pkg::inc(a);
    $display("package %0d %0d", b[0], b[3]);
    r = mk(8'h0f);
    $display("package_record %h %h", r.a, r.b);
    i0.put(b);
    i1.put(a);
    i1.put(pkg::inc(a));
    a = i0.get();
    b = i1.get();
    $display("interface %0d %0d %0d %0d %0d", a[0], a[3], b[0], i0.puts, i1.puts);
    c0.setm(a);
    c1.setm(a);
    c1.setm(b);
    a = c0.getm();
    b = c1.getm();
    $display("module %0d %0d %0d %0d", a[0], b[0], c0.ncalls, c1.ncalls);
    r = '{8'h01, 8'h02};
    c1.swap_rec(r);
    $display("module_ref %0d %0d", r.a, r.b);
    note();
    pkg::note();
    $display("package_static %0d", pkg::calls);
    $finish(0);
  end
endmodule
