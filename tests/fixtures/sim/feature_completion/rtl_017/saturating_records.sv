// IEEE 1800-2009 11.11, A.2.8: saturating 8-bit record arithmetic through
// operator overloads. Every operator here is illegal for the unpacked record,
// so each use calls its bound function exactly once.
package satpkg;
  typedef struct { logic signed [7:0] v; } sat8;

  function automatic sat8 sat(int s);
    sat8 r;
    if (s > 127) s = 127;
    else if (s < -128) s = -128;
    r.v = s[7:0];
    return r;
  endfunction

  function automatic sat8 sadd(sat8 a, sat8 b); return sat(a.v + b.v); endfunction
  function automatic sat8 saddi(sat8 a, int b); return sat(a.v + b); endfunction
  function automatic sat8 ssub(sat8 a, sat8 b); return sat(a.v - b.v); endfunction
  function automatic sat8 smul(sat8 a, sat8 b); return sat(a.v * b.v); endfunction
  function automatic sat8 sdiv(sat8 a, sat8 b);
    if (b.v == 0) return sat(a.v < 0 ? -128 : 127);
    return sat(a.v / b.v);
  endfunction
  function automatic sat8 smod(sat8 a, sat8 b);
    if (b.v == 0) return sat(0);
    return sat(a.v % b.v);
  endfunction
  function automatic sat8 spow(sat8 a, int e);
    sat8 r = sat(1);
    for (int i = 0; i < e; i++) r = sat(r.v * a.v);
    return r;
  endfunction
  function automatic sat8 sneg(sat8 a); return sat(-a.v); endfunction
  function automatic sat8 spos(sat8 a); return a; endfunction
  function automatic sat8 sinc(sat8 a); return sat(a.v + 1); endfunction
  function automatic sat8 sdec(sat8 a); return sat(a.v - 1); endfunction
  function automatic bit slt(sat8 a, sat8 b); return a.v < b.v; endfunction
  function automatic bit sle(sat8 a, sat8 b); return a.v <= b.v; endfunction
  function automatic bit sgt(sat8 a, sat8 b); return a.v > b.v; endfunction
  function automatic bit sge(sat8 a, sat8 b); return a.v >= b.v; endfunction
  function automatic bit seqi(sat8 a, int b); return a.v == b; endfunction
  function automatic bit snei(sat8 a, int b); return a.v != b; endfunction
  function automatic sat8 fromint(int i); return sat(i); endfunction
  function automatic sat8 fromreal(real r); return sat($rtoi(r)); endfunction
endpackage

module tb;
  import satpkg::*;

  bind + function sat8 sadd(sat8, sat8);
  bind + function sat8 saddi(sat8, int);
  bind - function sat8 ssub(sat8, sat8);
  bind * function sat8 smul(sat8, sat8);
  bind / function sat8 sdiv(sat8, sat8);
  bind % function sat8 smod(sat8, sat8);
  bind ** function sat8 spow(sat8, int);
  bind - function sat8 sneg(sat8);
  bind + function sat8 spos(sat8);
  bind ++ function sat8 sinc(sat8);
  bind -- function sat8 sdec(sat8);
  bind < function bit slt(sat8, sat8);
  bind <= function bit sle(sat8, sat8);
  bind > function bit sgt(sat8, sat8);
  bind >= function bit sge(sat8, sat8);
  bind == function bit seqi(sat8, int);
  bind != function bit snei(sat8, int);
  bind = function sat8 fromint(int);
  bind = function sat8 fromreal(real);

  sat8 a, b, c, d, e, z, r;
  sat8 arr [0:2];
  sat8 k = 100;
  int count;
  logic [3:0] n = 4'hF;
  bit [39:0] big = 40'h10_0000_0005;

  function automatic int show(sat8 x);
    return x.v;
  endfunction

  function automatic sat8 mk();
    return -5;
  endfunction

  initial begin
    a = 100; b = 50; c = -3; d = -128; e = -1; z = 0;
    r = a + b; $display("add %0d", r.v);
    r = c + c; $display("add2 %0d", r.v);
    r = d - b; $display("sub %0d", r.v);
    r = c - d; $display("sub2 %0d", r.v);
    r = a * c; $display("mul %0d", r.v);
    r = c * c; $display("mul2 %0d", r.v);
    r = d / c; $display("div %0d", r.v);
    r = d / e; $display("div2 %0d", r.v);
    r = a / z; $display("div0 %0d", r.v);
    r = d % c; $display("mod %0d", r.v);
    r = a % c; $display("mod2 %0d", r.v);
    r = c ** 3; $display("pow %0d", r.v);
    r = c ** 5; $display("pow2 %0d", r.v);
    r = -d; $display("neg %0d", r.v);
    r = -c; $display("neg2 %0d", r.v);
    r = +c; $display("pos %0d", r.v);
    r = (a + b) * c - d; $display("nest %0d", r.v);
    r = a + 20; $display("addi %0d", r.v);
    r = a + 4'd9; $display("addi2 %0d", r.v);
    $display("rel %0d %0d %0d %0d", a < b, a <= a, c > d, d >= c);
    $display("eqi %0d %0d %0d", a == 100, a != 100, c == -3);
    r = 125; r++; $display("inc %0d", r.v);
    ++r; r++; $display("inc2 %0d", r.v);
    r = -127; r--; --r; $display("dec %0d", r.v);
    arr[0] = 1; arr[1] = 2; arr[1]++; arr[2] = arr[0] + arr[1];
    $display("arr %0d %0d", arr[1].v, arr[2].v);
    r = 10; r += a; $display("comp %0d", r.v);
    r -= c; $display("comp2 %0d", r.v);
    r *= e; $display("comp3 %0d", r.v);
    r /= c; $display("comp4 %0d", r.v);
    r %= c; $display("comp5 %0d", r.v);
    r += 5; $display("comp6 %0d", r.v);
    $display("init %0d", k.v);
    r = 3.7; $display("real %0d", r.v);
    r = -300.5; $display("real2 %0d", r.v);
    r = n; $display("intx %0d", r.v);
    r = big; $display("big %0d", r.v);
    r = sat8'(300); $display("cast %0d", r.v);
    $display("arg %0d", show(200));
    r = mk(); $display("ret %0d", r.v);
    count = 0;
    for (r = 90; r < k; ++r) count++;
    $display("loop %0d %0d", count, r.v);
    $finish(0);
  end
endmodule
