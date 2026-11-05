// Assignment operators and increments through a run-time index: the left side
// is evaluated once (SV 11.4.1).
module tb;
  typedef struct { string s; int a[3]; real f[2]; logic [7:0] b[2]; } r_t;
  typedef struct { string n; r_t e[3]; } o_t;
  r_t r;
  o_t o;
  r_t nb[3];
  int k = 1, n = 0;
  function automatic int f(int v);
    n++;
    $display("f %0d", v);
    return v;
  endfunction
  task automatic t();
    r_t l;
    l.a[k] = 3;
    l.a[k] += 2;
    l.a[k]++;
    l.a[f(1)] *= f(3);
    l.f[k] = 1.5;
    l.f[k] *= 2.0;
    $display("%0d %f", l.a[1], l.f[1]);
  endtask
  initial begin
    r.a[1] = 10;
    r.a[k] += 1;
    $display("%0d", r.a[1]);
    r.a[k]++;
    --r.a[k];
    r.a[k] <<= 2;
    $display("%0d", r.a[1]);
    r.a[f(1)] += f(2);
    $display("%0d %0d", r.a[1], n);
    r.f[0] = 2.5;
    k = 0;
    r.f[k] *= 2.0;
    r.f[k]++;
    $display("%f", r.f[0]);
    r.b[k] = 8'h0f;
    r.b[k] ^= 8'hff;
    $display("%h", r.b[0]);
    $display("%0d", r.a[k]++);
    $display("%0d", r.a[0]);
    $display("%0d", ++r.a[k]);
    k = 7;
    r.a[k] += 1;
    r.a[k]++;
    $display("%0d %0d %0d", r.a[0], r.a[1], r.a[2]);
    k = 1;
    t();
    o.e[k].a[k] = 5;
    o.e[k].a[k] += 2;
    o.e[f(1)].a[f(1)]++;
    nb[k].a[k] = 6;
    nb[k].a[k] -= 2;
    nb[f(1)].a[f(1)] <<= 1;
    $display("%0d %0d", o.e[1].a[1], nb[1].a[1]);
    $finish(0);
  end
endmodule
