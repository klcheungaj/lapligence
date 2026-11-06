// Static random properties (18.4, 8.9): a static array and a static variable
// are one variable of the whole problem, whichever object names them, also
// through a `rand` handle; every object sees the solved value.
class w_c;
  static rand bit [3:0] sa[2];
  static rand bit [3:0] dq[];
  rand bit [3:0] x;
  constraint s { foreach (sa[i]) sa[i] == i + x; x < 5; dq.size() == 2; foreach (dq[i]) dq[i] == 7; }
endclass

class h_c;
  rand w_c w;
  function new(); w = new; endfunction
endclass

class s_c;
  rand bit [3:0] t;
  static rand bit [3:0] g;
  constraint c { g == t + 1; t < 3; }
endclass

class p_c;
  rand s_c s;
  rand bit [3:0] v;
  constraint c { v == s.g; }
  function new(); s = new; endfunction
endclass

module tb;
  initial begin
    static w_c v = new;
    static w_c u = new;
    static h_c h = new;
    static p_c p = new;
    int seen[5];
    int bad;
    repeat (100) begin
      if (v.randomize() != 1) $display("failed");
      if (w_c::sa[1] != v.x + 1 || w_c::sa[0] != v.x) bad++;
      if (u.dq.size() != 2 || w_c::dq[1] != 7) bad++;
      seen[v.x]++;
    end
    $display("bad=%0d", bad);
    foreach (seen[i]) if (seen[i] == 0) $display("missing %0d", i);
    $display("%0d %0d", u.randomize(), u.sa[0] == u.x);
    $display("%0d %0d", h.randomize(), w_c::sa[0] == h.w.x);
    $display("%0d %0d %0d", p.randomize(), s_c::g == p.s.t + 1, p.v == s_c::g);
    $finish(0);
  end
endmodule
