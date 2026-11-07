class it; int a; function new(int x=0); a=x; endfunction function void clear(); a=0; endfunction function int get(); return a; endfunction function it self_(); return this; endfunction endclass
function automatic it mk(int x); it i=new(x); return i; endfunction
class own #(type T=it);
  T b[$]; T da[]; T fa[2]; it m[string]; int ii[$]; string sq[$];
  it nest[$][$];
  function void go();
    b.push_back(mk(1)); b.push_back(mk(2)); b.push_back(mk(3));
    void'(b.pop_back()); $display("%0d", b.size());
    void'(b.pop_front()); $display("%0d", b.size());
    b.pop_back(); $display("%0d", b.size());
    b.push_back(mk(5));
    b[0].clear(); $display("%0d", b[0].a);
    void'(b[0].get());
    void'(b[0].self_());
    b[0].self_().clear();
    m["k"]=mk(3); m["k"].clear(); void'(m["k"].get());
    ii.push_back(1); void'(ii.pop_back());
    sq.push_back("x"); void'(sq.pop_back());
    if (b[0].get() == 0) $display("yes");
  endfunction
endclass
module tb;
  it fq[$]; 
  own #(it) o = new;
  initial begin
    o.go();
    fq.push_back(mk(9)); 
    fq[0].clear(); void'(fq.pop_back()); $display("%0d", fq.size());
    o.b[0].clear();
    void'(o.b.pop_back());
  end
endmodule
