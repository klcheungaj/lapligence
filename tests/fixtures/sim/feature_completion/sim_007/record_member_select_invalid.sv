// Run-time indices out of range or unknown read the default and write nothing.
module tb;
  typedef struct { string s[3]; int a[3]; logic [7:0] b; real r[2]; } el_t;
  typedef struct { string name; el_t e[3]; } rec_t;
  el_t nb[3];
  rec_t m;
  int k = 1, j = 7;
  integer x;
  initial begin
    nb[1].a[1] = 1;
    m.e[1].a[1] = 2;
    m.e[1].b = 8'h5a;
    nb[j].a[k] = 9;
    nb[k].a[j] = 9;
    m.e[j].a[k] = 9;
    m.e[k].a[j] = 9;
    m.e[k].s[j] = "no";
    m.e[j].r[k] = 9.0;
    $display("%0d %0d [%s] %0.1f", nb[k].a[j], m.e[k].a[j], m.e[k].s[j], m.e[j].r[k]);
    $display("%0d %0d %h", nb[1].a[1], m.e[1].a[1], m.e[1].b);
    $display("%h", m.e[j].b);
    x = 'x;
    $display("%0d %0d [%s]", nb[x].a[k], m.e[k].a[x], m.e[x].s[k]);
    m.e[k].a[x] = 9;
    m.e[x].a[k] = 9;
    m.e[k].b[x] = 1'b1;
    $display("%0d %h", m.e[1].a[1], m.e[1].b);
    $finish(0);
  end
endmodule
