class C; int a; function new(int x=0); a=x; endfunction endclass
class cmap;
  C m[C]; C qs[$];
  function C get(C k); if (m.exists(k)) return m[k]; return null; endfunction
  function int nothing(); return 0; endfunction
  function C find(string s, int d); return (s=="x") ? null : qs[0]; endfunction
  static function cmap inst(); static cmap i; if (i==null) i=new; return i; endfunction
endclass
class D extends C; function new(int x=0); super.new(x); endfunction endclass
module tb;
  cmap g = new; C a=new(1), b=new(2); D d=new(3); C r;
  virtual class VC; pure virtual function C pv(); endclass
  function C fc(); return a; endfunction
  initial begin
    g.m[a]=b; g.qs.push_back(a);
    if (g.get(a) != null) $display("1");
    if (g.get(b) == null) $display("2");
    if (null != g.get(a)) $display("3");
    if (g.find("x",0) == null) $display("4");
    if (g.find("y",0) == a) $display("5");
    if (g.find("y",0) != g.find("x",0)) $display("6");
    if (g.get(a)) $display("7");
    if (!g.get(b)) $display("8");
    if (fc() === a) $display("10");
    if (cmap::inst() != null) $display("11");
    if (cmap::inst().get(a) == null) $display("12");
    $display("%0d", g.get(a) == b);
    $display("%0d", g.get(a) == g.get(a));
    r = (g.get(a) == null) ? a : g.get(a); $display("%0d", r.a);
    r = (g.m.exists(a)) ? g.m[a] : null; $display("%0d", r.a);
    r = (g.m.exists(b)) ? g.m[b] : d; $display("%0d", r.a);
    r = g.m.exists(a) ? ((a==b) ? d : g.get(a)) : null; $display("%0d", r.a);
    g.qs.push_back(g.m.exists(a) ? a : b); $display("%0d", g.qs[1].a);
    while (g.get(b) != null) begin end
    for (int i=0; g.get(a) != null && i<2; i++) $display("l");
    assert (g.get(a) != null);
    do r=null; while (g.get(b) != null);
    $display("%s", g.get(a) == null ? "n" : "nn");
    r = g.get(a); 
    if (r == g.get(a) || 0) $display("14");
  end
endmodule
