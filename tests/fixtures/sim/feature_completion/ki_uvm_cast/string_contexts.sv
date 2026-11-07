class C;
  int v;
  function new(int x = 0);
    v = x;
  endfunction
endclass
class K;
  string nm = "a";
  C pool[string];
  function string get_name();
    return nm;
  endfunction
  function C lookup();
    return pool[get_name()];
  endfunction
  function bit known();
    return pool.exists(get_name());
  endfunction
  function string state_of(process p);
    return p.get_randstate();
  endfunction
  function void keep(process p, string s);
    p.set_randstate(state_of(p));
    p.set_randstate(s);
  endfunction
endclass
module tb;
  K k = new;
  function automatic string text(real r, bit b);
    string s;
    s = {"time ", $sformatf("%0.1f", r), " end"};
    $display("%s", s);
    return b ? {"yes ", $sformatf("%0.2f", r)} : "no";
  endfunction
  function automatic string name_of(K h);
    return h.get_name();
  endfunction
  function automatic int value_of(K h);
    C c = h.pool[name_of(h)];
    return c.v;
  endfunction
  initial begin
    process p;
    C c;
    p = process::self();
    c = new(5);
    k.pool["a"] = c;
    $display("%s", text(1.5, 1));
    $display("%s", text(2.5, 0));
    $display("%0d %0d", k.lookup().v, k.known());
    $display("%0d", value_of(k));
    k.keep(p, p.get_randstate());
    $finish;
  end
endmodule
