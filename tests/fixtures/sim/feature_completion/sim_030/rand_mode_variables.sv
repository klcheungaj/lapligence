// rand_mode on class handle variables: inside a method with the implicit
// `this`, through a nested handle, and with a loop index.
class leaf;
  rand int x;
  constraint c { x inside {[1:100000]}; }
endclass
class node;
  rand leaf l[4];
  rand leaf one;
  function new();
    foreach (l[i]) l[i] = new;
    one = new;
  endfunction
  function void quiet_odd();
    for (int i = 1; i < 4; i += 2) l[i].rand_mode(0);
  endfunction
  function void quiet_one();
    one.rand_mode(0);
  endfunction
endclass
class top_node;
  rand node n;
  function new();
    n = new;
  endfunction
endclass
module tb;
  top_node t = new;
  int old[4];
  int one_old;
  int ok;
  function automatic void snap();
    foreach (old[i]) old[i] = t.n.l[i].x;
    one_old = t.n.one.x;
  endfunction
  function automatic void show(string what);
    $write("%s:", what);
    foreach (old[i]) $write(" %0d", old[i] != t.n.l[i].x);
    $display(" one %0d", one_old != t.n.one.x);
  endfunction
  initial begin
    ok = t.randomize();
    snap();
    ok = t.randomize();
    show("all");
    t.n.quiet_odd();
    snap();
    ok = t.randomize();
    show("odd off");
    t.n.quiet_one();
    t.n.l[0].rand_mode(0);
    snap();
    ok = t.randomize();
    show("one off");
    t.n.l.rand_mode(1);
    t.n.one.rand_mode(1);
    snap();
    ok = t.randomize();
    show("all on");
    $finish;
  end
endmodule
