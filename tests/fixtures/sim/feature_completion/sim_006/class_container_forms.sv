// Class container forms the UVM libraries use: `this.q`, string and ref keys
// of traversal methods, class methods named like enum methods (SV 7.9, 8.6).
module tb;
  class B;
    int in_use[$];
    protected int m_last[$];
    string names[string];
    int ids[string];
    bit blks[int];
    function void put(int v);
      this.m_last.push_front(v);
      if (this.m_last.size() > 2) this.m_last.delete(2);
    endfunction
    function int cnt();
      return this.in_use.size();
    endfunction
  endclass
  class D extends B;
    function void fill();
      this.put(3);
      this.in_use.push_back(1);
      for (int i = 0; i < this.in_use.size(); i++) $display("%0d", this.in_use[i]);
      if (this.blks.exists(1)) $display("yes");
    endfunction
    function void cp(D o);
      o.in_use = this.in_use;
    endfunction
    function bit f1(ref string key);
      return ids.first(key);
    endfunction
    function bit f2(output string key);
      string k;
      if (ids.first(k)) begin key = k; return 1; end
      return 0;
    endfunction
    function void scan();
      string l;
      if (ids.first(l)) do $display("%s", l); while (ids.next(l));
    endfunction
  endclass
  class P #(type K = int, type T = int);
    protected T pool[K];
    function int first(ref K key);
      return pool.first(key);
    endfunction
    function int next(ref K key);
      return pool.next(key);
    endfunction
    function void add(K k, T v);
      pool[k] = v;
    endfunction
    function int num(); return pool.num(); endfunction
    function void clr(K k); pool.delete(k); endfunction
    function int ex(K k); return pool.exists(k); endfunction
    function T get(K k); return pool[k]; endfunction
  endclass
  initial begin
    automatic D d = new, e = new;
    automatic P#(string, int) p = new;
    automatic P#(int, string) p2 = new;
    automatic string s;
    automatic int k;
    d.fill();
    d.cp(e);
    $display("%0d", e.cnt());
    d.ids["a"] = 1;
    d.ids["b"] = 2;
    d.scan();
    $display("%0d %s", d.f1(s), s);
    s = "";
    $display("%0d %s", d.f2(s), s);
    p.add("x", 3);
    p.add("y", 4);
    if (p.first(s)) do $display("%s %0d", s, p.get(s)); while (p.next(s));
    p2.add(4, "w");
    if (p2.first(k)) $display("%0d %s", k, p2.get(k));
    $finish;
  end
endmodule
