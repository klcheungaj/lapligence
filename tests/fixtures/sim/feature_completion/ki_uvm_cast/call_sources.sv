class base; int a; function new(int x=0); a=x; endfunction
  virtual function base clone(); base b=new(a+1); return b; endfunction
  virtual function string name(); return "base"; endfunction
endclass
class der extends base; int d; function new(int x=0); super.new(x); d=x*2; endfunction
  virtual function base clone(); der b=new(a+10); return b; endfunction
  virtual function string name(); return "der"; endfunction
endclass
class other extends base; virtual function string name(); return "other"; endfunction endclass
class q #(type T=int);
  T v[$];
  virtual function T get(int i); if (i>=v.size()) begin T t; return t; end return v[i]; endfunction
  function void add(T x); v.push_back(x); endfunction
  static function T mk(T x); return x; endfunction
endclass
class fac; 
  virtual function base create(string n); der d; other o; if (n=="der") begin d=new(5); return d; end  if (n=="other") begin o=new; return o; end return null; endfunction
endclass
module tb;
  base b; der d; other o; fac f = new; q#(base) qq=new;
  base arr[3]; der darr[2];
  function automatic bit pick(ref der r, input base s); return $cast(r, s); endfunction
  initial begin
    if ($cast(d, f.create("der"))) $display("ok1 %0d %s", d.a, d.name()); else $display("fail1");
    if ($cast(d, f.create("other"))) $display("ok2"); else $display("fail2 %0d", d.a);
    if ($cast(o, f.create("nope"))) $display("ok3 %0d", o==null); else $display("fail3");
    b = f.create("der");
    qq.add(b); qq.add(f.create("other"));
    for (int i=0;i<3;i++) if ($cast(d, qq.get(i))) $display("cast %0d ok %0d", i, d==null); else $display("cast %0d fail", i);
    $cast(d, b.clone()); $display("%0d %s", d.a, d.name());
    $cast(d, qq.mk(b)); $display("%0d", d.a);
    $cast(d, (1==1) ? b : null); $display("%0d", d.a);
    if (!$cast(darr[1], f.create("der"))) $display("no"); $display("%0d", darr[1].a);
    $cast(arr[2], darr[1]); $display("%s", arr[2].name());
    $cast(d, arr[2]); $display("%0d", d.a);
    $display("%0d", pick(d, f.create("other")));
  end
endmodule
