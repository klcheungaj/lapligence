// Run-time indices below a record element and through nested member arrays:
// module records, fixed-array elements and subroutine records, on every leaf
// kind. Selects inside packed members, descending and two-dimensional arrays.
module tb;
  class C; int v; function new(int x); v = x; endfunction endclass
  typedef struct {
    string s[3];
    int a[3];
    logic [7:0] b;
    logic [7:0] w[4:1];
    real r[2];
    C h[2];
    int g[1:0][0:2];
  } el_t;
  typedef struct { string name; el_t e[3]; } rec_t;
  typedef struct { string name; el_t e[1:3]; } asc_t;
  el_t nb[3];
  rec_t m;
  asc_t u;
  int k = 1, j = 2, n = 0;
  function automatic int f(int v);
    n++;
    $display("f %0d", v);
    return v;
  endfunction
  task automatic sub(int i, int l);
    rec_t r;
    el_t a[3];
    r.e[i].a[l] = 3;
    r.e[i].s[l] = "sub";
    r.e[i].b = 8'h0;
    r.e[i].b[l] = 1'b1;
    r.e[i].b[l + 1] = 1'b1;
    r.e[i].r[i - 1] = 0.5;
    r.e[i].h[i - 1] = new(21);
    r.e[i].g[i][l] = 22;
    a[i].a[l] = 4;
    a[i].b = 8'h0;
    a[1].b[l] = 1'b1;
    a[i].b[l + 1] = 1'b1;
    $display("%0d %s %h %0.1f %0d %0d", r.e[1].a[2], r.e[1].s[2], r.e[1].b, r.e[1].r[0],
             r.e[i].h[i - 1].v, r.e[i].g[1][2]);
    $display("%0d %h %0d", a[1].a[2], a[1].b, a[i].a[l]);
  endtask
  initial begin
    nb[1].a[k] = 5;
    nb[k].a[j] = 6;
    $display("%0d %0d %0d", nb[1].a[1], nb[1].a[2], nb[k].a[k]);
    nb[k].s[j] = "el";
    nb[j].r[k] = 1.5;
    nb[k].h[k - 1] = new(9);
    $display("%s %0.1f %0d", nb[1].s[2], nb[j].r[k], nb[k].h[k - 1].v);
    m.e[k].a[j] = 7;
    m.e[k].a[0] = 8;
    $display("%0d %0d %0d", m.e[1].a[2], m.e[k].a[j], m.e[k].a[0]);
    m.e[k].s[j] = "nest";
    m.e[j].r[k] = 2.5;
    m.e[k].h[j - 1] = new(10);
    $display("%s %0.1f %0d", m.e[1].s[2], m.e[j].r[k], m.e[k].h[j - 1].v);
    m.e[1].b = 8'h0;
    m.e[k].b[2] = 1'b1;
    m.e[k].b[j + 1] = 1'b1;
    $display("%h %b %b", m.e[1].b, m.e[k].b[2], m.e[k].b[j]);
    m.e[k].b[7:4] = 4'ha;
    $display("%h %h", m.e[1].b, m.e[k].b[7:4]);
    m.e[k].b[j +: 2] = 2'b01;
    $display("%h %b", m.e[1].b, m.e[k].b[j +: 2]);
    m.e[j].w[k] = 8'h3c;
    m.e[j].w[k][j] = 1'b0;
    $display("%h %b", m.e[2].w[1], m.e[j].w[k][j + 1]);
    nb[1].b = 8'h0;
    nb[k].b[j] = 1'b1;
    nb[k].b[j +: 2] = 2'b11;
    $display("%h %b", nb[1].b, nb[k].b[j]);
    // A two-dimensional member array has one run-time position per dimension.
    m.e[k].g[k][j] = 31;
    nb[k].g[0][j] = 32;
    $display("%0d %0d %0d", m.e[1].g[1][2], nb[1].g[0][2], m.e[k].g[k][j]);
    // Ascending bounds of the element array.
    u.e[k].a[j] = 41;
    u.e[j + 1].a[0] = 42;
    $display("%0d %0d %0d", u.e[1].a[2], u.e[3].a[0], u.e[k].a[j]);
    // Subroutine records, by value through a task with run-time indices.
    sub(k, j);
    // Each index runs once, the value first, then left to right.
    m.e[f(1)].a[f(2)] = f(9);
    $display("%0d %0d", m.e[1].a[2], n);
    n = 0;
    $display("%0d %0d", m.e[f(1)].a[f(2)], n);
    nb[f(1)].a[f(2)] = f(3);
    $display("%0d", nb[1].a[2]);
    m.e[1].b = 8'h0;
    m.e[f(1)].b[f(3)] = 1'b1;
    $display("%h", m.e[1].b);
    $finish(0);
  end
endmodule
