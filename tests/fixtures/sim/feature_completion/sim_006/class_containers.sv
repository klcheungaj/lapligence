// SIM-006 A01: resizable containers as class properties (SV 8.5, 8.9, 7.5,
// 7.8, 7.10). Each object owns its instance containers; a static property
// is one container shared by every object; derived classes reach inherited
// containers through `this`.
module tb;
  class Base;
    int q[$];
    string names[string];
    static int shared[$];
    function void add(int v);
      q.push_back(v);
      names[$sformatf("k%0d", v)] = "v";
      shared.push_back(v * 10);
    endfunction
    function int total();
      return q.sum();
    endfunction
    function int drop_front();
      int x = q.pop_front();
      q.delete();
      return x;
    endfunction
  endclass
  class Derived extends Base;
    real r[];
    function void more();
      add(5);
      r = new[2];
      r[1] = 1.5;
      q[0] = q[0] + 100;
    endfunction
    function string report();
      return $sformatf("%0d %0d %0d %.1f %0d", q.size(), total(), names.num(), r[1],
                       shared.size());
    endfunction
  endclass
  Base k, k2, alias_k;
  Derived d;
  initial begin
    k = new;
    k.add(3);
    k.add(4);
    k2 = new;
    k2.add(9);
    alias_k = k;
    alias_k.add(1);
    d = new;
    d.more();
    $display("class total=%0d %0d", k.total(), k2.total());
    $display("derived %s", d.report());
    $display("static %0d %0d", Base::shared.size(), Base::shared[4]);
    $display("drop %0d %0d %0d", k.drop_front(), k.total(), k2.total());
    $finish(0);
  end
endmodule
