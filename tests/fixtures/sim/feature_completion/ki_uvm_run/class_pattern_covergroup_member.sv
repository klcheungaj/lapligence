// Class properties beside embedded covergroups print in an assignment
// pattern: the covergroup instances are left out, as a library's pool of
// monitors prints its entries.
class pool #(type T = int);
  T p[string];
  function string show();
    string v, k, all;
    if (p.first(k)) do begin
      $swrite(v, "%p", p[k]);
      if (all.len() > 0) all = {all, " "};
      all = {all, k, "=", v};
    end while (p.next(k));
    return all;
  endfunction
endclass

class mon;
  int x = 7;
  int kind = 3;
  covergroup cov_x;
    coverpoint x { bins lo = {[0:7]}; bins hi = {[8:15]}; }
  endgroup
  string name = "m";
  covergroup cov_kind;
    coverpoint kind;
  endgroup
  function new();
    cov_x = new;
    cov_kind = new;
  endfunction
  function void hit(int v);
    x = v;
    cov_x.sample();
    cov_kind.sample();
  endfunction
endclass

class slave_mon extends mon;
  int beats = 2;
  covergroup cov_beats;
    coverpoint beats;
  endgroup
  function new();
    super.new();
    cov_beats = new;
  endfunction
endclass

class env;
  mon m;
  int id = 1;
endclass

module tb;
  int y = 3;
  covergroup cg_y;
    coverpoint y;
  endgroup
  cg_y cy = new;
  initial begin
    pool #(mon) q = new;
    mon m = new;
    slave_mon s = new;
    env e = new;
    m.hit(9);
    s.hit(2);
    cy.sample();
    e.m = s;
    q.p["a"] = m;
    q.p["b"] = s;
    $display("%s", q.show());
    $display("id=%0d", e.id);
    $display("%p", e.m);
  end
endmodule
