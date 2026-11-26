// Static `rand` properties of a class that `rand` handles reach (readme:
// "Random properties").
typedef struct packed {
  logic [3:0] hi, lo;
} pair_t;
class st;
  rand bit [3:0] t;
  static rand bit [3:0] g;
  static rand pair_t k;
  static rand bit [3:0] fa[2];
  static rand bit [3:0] da[];
  constraint c {
    t < 3;
    g == t + 1;
    k.hi == t;
    k.lo == 5;
    foreach (fa[i]) fa[i] == t + i;
    da.size() == 2;
    foreach (da[i]) da[i] == 7 + t;
  }
endclass
class mid;
  rand st s = new;
endclass
class direct;
  rand st s = new;
  st other = new;
  rand bit [3:0] v[8];
  constraint c {
    v[0] == s.g;
    v[1] == st::g;
    v[2] == other.g;
    v[3] == s.k.hi;
    v[4] == {3'b0, st::k[0]};
    v[5] == s.fa[1];
    v[6] == st::da[1];
    v[7] == s.da.size();
  }
endclass
class through;
  rand mid m = new;
  rand bit [3:0] v[2];
  constraint c {
    v[0] == m.s.g;
    v[1] == st::k.hi;
  }
endclass
class state;
  st s = new;
  rand bit [3:0] v[2];
  constraint c {
    v[0] == s.g;
    v[1] == st::k.lo;
  }
endclass
module tb;
  direct d = new;
  through t = new;
  state n = new;
  int ok;
  task show(string what, st s);
    $display("%s %0d: %b %b %b %b", what, ok, st::g == s.t + 1, st::k == {s.t, 4'd5},
             st::fa[1] == s.t + 1, st::da[1] == s.t + 7);
  endtask
  task set();
    st::g = 9;
    st::k = 8'h76;
    st::fa = '{11, 12};
    st::da = '{13, 14, 15};
  endtask
  initial begin
    repeat (3) begin
      ok = d.randomize();
      show("direct", d.s);
      $display("  %b %b %b %b %b %b %b %b", d.v[0] == st::g, d.v[1] == st::g, d.v[2] == st::g,
               d.v[3] == st::k.hi, d.v[4] == 1, d.v[5] == st::fa[1], d.v[6] == st::da[1],
               d.v[7] == 2);
      ok = t.randomize();
      show("through", t.m.s);
      $display("  %b %b", t.v[0] == st::g, t.v[1] == st::k.hi);
    end
    set();
    ok = n.randomize();
    $display("state %0d: %0d %0d %0d %h", ok, n.v[0], n.v[1], st::g, st::k);
    // No random object of the class: its static variables are state.
    t.m.s = null;
    set();
    ok = t.randomize();
    $display("null %0d: %0d %0d, %0d %h %0d %0d", ok, t.v[0], t.v[1], st::g, st::k, st::fa[1],
             st::da.size());
    d.s.rand_mode(0);
    set();
    ok = d.randomize() with {v[7] inside {2, 3};};
    $display("off %0d: %0d %0d, %0d %h %0d %0d", ok, d.v[0], d.v[6], st::g, st::k, st::fa[1],
             st::da.size());
  end
endmodule
