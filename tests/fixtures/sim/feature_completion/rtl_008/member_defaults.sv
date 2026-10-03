// SV2009 6.8, 7.2.1-7.2.2, 7.4, 13.5.3: unpacked structure members keep their
// explicit defaults through nesting, arrays of records and declaration
// assignments; members without one start at their type's uninitialized value
// (two-state 0, four-state X). Default arguments are evaluated per call, and
// nested defaults in declaration initializers run once each, in order.
module tb;
  typedef struct { bit [3:0] b; logic [3:0] l; int i = 7; } inner_t;
  typedef struct { inner_t in; byte k = 8'h5a; bit [1:0] z; integer w; } outer_t;
  typedef struct { inner_t row [0:1]; shortint s = -3; } holder_t;
  outer_t o;
  inner_t arr [0:1];
  holder_t h;
  outer_t assigned = '{in: '{b: 4'h3, l: 4'hc, i: 9}, k: 8'h01, z: 2'b10, w: 5};
  int hits = 0;
  function automatic int h3(int b = 2);
    hits++;
    return b * 3;
  endfunction
  function automatic int g(int a = h3(), int c = h3(4));
    return a + c;
  endfunction
  int v1 = g();
  int v2 = g(.c(1));
  int v3 = g(10, h3());
  initial begin
    $display("%b %b %0d %h %b %b", o.in.b, o.in.l, o.in.i, o.k, o.z, o.w === 32'bx);
    $display("%b %b %0d %b %b %0d", arr[0].b, arr[0].l, arr[0].i, arr[1].b, arr[1].l, arr[1].i);
    $display("%b %b %0d %0d", h.row[1].b, h.row[1].l, h.row[0].i, h.s);
    $display("%h %h %0d %h %b %0d", assigned.in.b, assigned.in.l, assigned.in.i, assigned.k, assigned.z, assigned.w);
    $display("%0d %0d %0d %0d", v1, v2, v3, hits);
    $finish(0);
  end
endmodule
