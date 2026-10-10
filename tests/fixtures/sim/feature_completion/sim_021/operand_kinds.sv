// IEEE 1800-2009 11.11: overloads on an unpacked union, on a record with
// real and shortreal members combined with real and shortreal operands (the
// clause's float example), and on mixed record/int/string operands. A byte
// operand reaches the only prototype with an integral formal through the
// normal implicit cast; the other formals match exactly.
module tb;
  typedef union { int i; bit [31:0] b; } u_t;
  typedef struct { real r; shortreal s; } f_t;
  typedef struct { int v; string tag; } m_t;

  function automatic u_t uadd(u_t a, u_t b);
    uadd.i = a.i + b.i;
  endfunction
  function automatic bit ueq(u_t a, int b);
    return a.i == b;
  endfunction

  function automatic f_t faddfr(f_t a, real b);
    faddfr.r = a.r + b;
    faddfr.s = a.s;
  endfunction
  function automatic f_t faddfs(f_t a, shortreal b);
    faddfs.r = a.r;
    faddfs.s = a.s + b;
  endfunction
  function automatic f_t faddff(f_t a, f_t b);
    faddff.r = a.r + b.r;
    faddff.s = a.s + b.s;
  endfunction
  function automatic f_t fcopyr(real r);
    fcopyr.r = r;
    fcopyr.s = 0.5;
  endfunction
  function automatic f_t fneg(f_t a);
    fneg.r = -a.r;
    fneg.s = -a.s;
  endfunction
  function automatic bit flt(f_t a, f_t b);
    return a.r < b.r;
  endfunction

  function automatic m_t addmi(m_t a, int b);
    addmi.v = a.v + b;
    addmi.tag = {a.tag, "i"};
  endfunction
  function automatic m_t addim(int a, m_t b);
    addim.v = a + b.v;
    addim.tag = {"i", b.tag};
  endfunction
  function automatic m_t addms(m_t a, string s);
    addms.v = a.v;
    addms.tag = {a.tag, s};
  endfunction

  bind + function u_t uadd(u_t, u_t);
  bind == function bit ueq(u_t, int);
  bind + function f_t faddfr(f_t, real);
  bind + function f_t faddfs(f_t, shortreal);
  bind + function f_t faddff(f_t, f_t);
  bind = function f_t fcopyr(real);
  bind - function f_t fneg(f_t);
  bind < function bit flt(f_t, f_t);
  bind + function m_t addmi(m_t, int);
  bind + function m_t addim(int, m_t);
  bind + function m_t addms(m_t, string);

  u_t ux, uy, uz;
  f_t a, b;
  shortreal sr;
  m_t x, y;
  byte b8;
  string s;

  initial begin
    ux.i = 3;
    uy.i = 4;
    uz = ux + uy;
    $display("union %0d %0d %0d", uz.i, uz == 7, uz == 8);
    a = 1.25;
    $display("conv %0.3f %0.3f", a.r, a.s);
    b = a + 2.5;
    $display("real %0.3f %0.3f", b.r, b.s);
    sr = 0.25;
    b = b + sr;
    $display("shortreal %0.3f %0.3f", b.r, b.s);
    b = a + b;
    $display("record %0.3f %0.3f %0d %0d", b.r, b.s, a < b, b < a);
    b = -b;
    $display("neg %0.3f %0.3f", b.r, b.s);
    x.v = 1;
    x.tag = "x";
    b8 = 5;
    s = "S";
    y = x + 2;
    $display("mi %0d %s", y.v, y.tag);
    y = 3 + x;
    $display("im %0d %s", y.v, y.tag);
    y = x + b8;
    $display("mbyte %0d %s", y.v, y.tag);
    y = x + s;
    $display("ms %0d %s", y.v, y.tag);
    $finish;
  end
endmodule
