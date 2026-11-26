// Decision EO-D3: the actuals of a subroutine call are evaluated left to
// right, and an inout actual is copied in, and a `ref` actual bound, at its
// place among them: a call in an earlier input actual that changes what an
// inout or `ref` actual names (`tr2(bump(), arr[k])`) moves it. An output or
// inout actual is located again when it is copied out. This holds for
// functions and tasks, static and automatic ones, methods, string actuals
// and actuals through virtual interfaces.
//
// IEEE 1800-2009 13.5 (SystemVerilog-1800-2009.txt L19070): "The order of
//   evaluation of the expressions in the argument list is undefined."
// IEEE 1800-2009 13.5.2 (L19185-19187): "an inout argument is copied twice:
//   once from the actual into the argument when the subroutine is called and
//   once from the argument into the actual when the subroutine returns."
interface ifc;
  string s[2];
endinterface
class C;
  int v;
  int base;
  task automatic m_io(input int w, inout int r);
    r = r + w + base;
  endtask
endclass
module tb;
  typedef struct { int x; int y; } pt;
  ifc i();
  ifc j();
  virtual ifc va[2];
  int arr[4];
  string s[2];
  real r[2];
  pt p[2];
  int k, x, y, n;
  C o;
  C ca[2];
  function automatic int bump();
    k = 1;
    n++;
    return 1;
  endfunction
  function automatic string sbump();
    k = 1;
    return "x";
  endfunction
  function automatic int sety(int v);
    y = v;
    return v;
  endfunction
  function automatic int gi();
    n++;
    return k;
  endfunction
  function automatic int f2(int a, int b);
    return a * 10 + b;
  endfunction
  task automatic tr(inout int r, input int w);
    r = r + w;
  endtask
  task automatic tr2(input int w, inout int r);
    r = r + w;
  endtask
  task tr2_static(input int w, inout int r);
    r = r + w;
  endtask
  function automatic int fr2(input int w, inout int r);
    r = r + w;
    return r;
  endfunction
  task automatic tref2(input int w, ref int r);
    r = r + w;
  endtask
  task automatic tio3(input int w, inout int r, input int z);
    r = r * 100 + w * 10 + z;
  endtask
  task automatic tstr(input int w, inout string r);
    r = {r, "!"};
  endtask
  task automatic ts_io(input string w, inout int r);
    r = r + w.len();
  endtask
  task automatic treal(input int w, inout real r);
    r = r + w;
  endtask
  task automatic tpt(input int w, inout pt r);
    r.x = r.x + w;
  endtask
  task automatic tio2(inout int a, inout int b);
    a = a + 1; b = b + 2;
  endtask
  task automatic to2(input int w, output int r);
    r = w + 40;
  endtask
  task automatic reset();
    arr = '{10, 20, 30, 40};
    s = '{"s0", "s1"};
    r = '{1.5, 2.5};
    p[0] = '{1, 2}; p[1] = '{3, 4};
    va[0] = i; va[1] = j;
    i.s = '{"i0", "i1"}; j.s = '{"j0", "j1"};
    k = 0; x = 0; y = 0; n = 0;
    o.v = 0; o.base = 0; ca[0].v = 100; ca[1].v = 200;
  endtask
  initial begin
    o = new; ca[0] = new; ca[1] = new;
    reset(); tr2(bump(), arr[k]);
    $display("A %p", arr);
    reset(); tr(arr[k], bump());
    $display("B %p", arr);
    reset(); x = fr2(bump(), arr[k]);
    $display("C %0d %p", x, arr);
    reset(); tr2(sety(5), y);
    $display("D y=%0d", y);
    reset(); tref2(bump(), arr[k]);
    $display("E %p", arr);
    reset(); tio3(bump(), arr[k], sety(3) + arr[k]);
    $display("F %p", arr);
    reset(); tstr(bump(), s[k]);
    $display("G %p", s);
    reset(); treal(bump(), r[k]);
    $display("H %p", r);
    reset(); tpt(bump(), p[k]);
    $display("I %p", p);
    reset(); tr2(bump(), ca[k].v);
    $display("J %0d %0d", ca[0].v, ca[1].v);
    reset(); tr(arr[gi()], bump());
    $display("K n=%0d %p", n, arr);
    reset(); tio2(arr[k], arr[bump()]);
    $display("L %p", arr);
    reset(); x = fr2(1, arr[k]) + bump();
    $display("M %0d %p", x, arr);
    reset(); to2(sety(7), y);
    $display("N y=%0d", y);
    reset(); tr2((k = 1), arr[k]);
    $display("O %p", arr);
    reset(); ts_io(sbump(), arr[k]);
    $display("P %p", arr);
    reset(); tr2_static(bump(), arr[k]);
    $display("Q %p", arr);
    reset(); o.m_io(bump(), arr[k]);
    $display("R %p", arr);
    reset(); x = f2(fr2(bump(), arr[k]), arr[k]);
    $display("S %0d %p", x, arr);
    reset(); x = arr[k] + fr2(bump(), arr[k]);
    $display("T %0d %p", x, arr);
    reset(); tstr(bump(), va[k].s[1]);
    $display("U %p %p", i.s, j.s);
    $finish;
  end
endmodule
