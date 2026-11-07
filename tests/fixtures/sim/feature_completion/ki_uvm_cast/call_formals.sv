class base; int a; function new(int x=0); a=x; endfunction
  function base me(ref int i, input int d); i += d; return this; endfunction
  function base me2(output int i, output string s, output real r, output bit [99:0] w, output int arr[3], output base other); i = a; s = "s"; r=1.5; w=100'hf; arr='{1,2,3}; other=this; return this; endfunction
  function base me3(output int q[$], ref string sq[$], ref int aa[string], ref byte b); q={1,2}; sq={"x"}; aa["k"]=9; b=3; return this; endfunction
  function base me4(ref int arr[3], const ref int cr, ref real r, ref string s); arr[1]=5; r=r+cr; s={s,"!"}; return this; endfunction
  function base me5(inout int i, inout string s, inout logic [3:0] n); i++; s={s,"i"}; n=~n; return this; endfunction
  static function base st(ref int i); i=99; return null; endfunction
endclass
typedef enum {R=1,G=2} col_t;
typedef struct packed { int a; byte b; } ps_t;
typedef struct { int a; string s; } us_t;
class base2 extends base; function new(int x=0); super.new(x); endfunction function base me6(output col_t c, output ps_t p, output us_t u); c=G; p='{1,2}; u='{3,"u"}; return this; endfunction endclass
module tb;
  function automatic base ff(ref int i, output int o); o=i; i=i+1; return null; endfunction
  function base fs(output int o); o=5; return null; endfunction
  base2 b=new(4), r; int i=1, j; string s; real rr=1.0; bit [99:0] w; int arr[3]; base o; int q[$]; string sq[$]; int aa[string]; byte by; logic [3:0] n=4'b1010; 
  int iq[$]={1,2}; col_t c; ps_t p; us_t u; int si; int fa[2]; 
  initial begin
    r = b; 
    if (b.me(i, 2) != null) $display("%0d", i);
    void'(b.me2(j, s, rr, w, arr, o)); $display("%0d %s %f %h %0d %0d %0d %0d", j, s, rr, w, arr[0], arr[1], arr[2], o.a);
    void'(b.me3(q, sq, aa, by)); $display("%0d %0d %s %0d %0d", q[0], q[1], sq[0], aa["k"], by);
    void'(b.me4(arr, i, rr, s)); $display("%0d %f %s", arr[1], rr, s);
    void'(b.me5(i, s, n)); $display("%0d %s %b", i, s, n);
    void'(base::st(i)); $display("%0d", i);
    void'(ff(i, j)); $display("%0d %0d", i, j);
    void'(fs(j)); $display("%0d", j);
    void'(b.me6(c, p, u)); $display("%0d %0d %0d %0d %s", c, p.a, p.b, u.a, u.s);
    if (b.me(iq[0], 3) == b) $display("%0d %0d", iq[0], iq[1]);
    void'(b.me(fa[1], 3)); $display("%0d %0d", fa[0], fa[1]);
    void'(b.me2(iq[1], s, rr, w, arr, o)); $display("%0d %0d", iq[0], iq[1]);
    if (ff(i, si) == null && si == 99) $display("y");
    void'(b.me2(p.a, s, rr, w, arr, o)); $display("%0d", p.a);
    void'(b.me2(j, sq[0], rr, w, arr, o)); $display("%s", sq[0]);
    void'(b.me(b.a, 1)); $display("%0d", b.a);
  end
endmodule
