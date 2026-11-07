class C; int a=1; endclass
class D extends C; int b=2; endclass
class P #(type T=int); T p[string]; function string s(string k); string v; $swrite(v, p[k]); return v; endfunction endclass
module tb;
  C c; C n; D d; string s;
  initial begin
    n = null; c = new; d = new;
    $swrite(s, c); $display("[%s]", s);
    $swrite(s, d); $display("[%s]", s);
    $swriteh(s, c); $display("[%s]", s);
    $swriteb(s, c); $display("[%s]", s);
    $display(c);
    $display(d);
    $write(c, "\n");
    $swrite(s, n); $display("[%s]", s);
    $swrite(s, "x", n, "y"); $display("[%s]", s);
    $swriteb(s, n); $display("[%s]", s);
    $swriteh(s, n); $display("[%s]", s);
    $swriteo(s, n); $display("[%s]", s);
    begin P#(C) q=new; q.p["a"]=null; $display("[%s]", q.s("a")); end
    begin P#(int) q=new; q.p["a"]=5; $display("[%s]", q.s("a")); end
    $display("n=", n);
    $display(n);
    $write(n, "\n");
  end
endmodule
