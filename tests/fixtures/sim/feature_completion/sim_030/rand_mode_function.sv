// The function form of rand_mode on class handle variables: inside a method
// through `this`, through a nested handle, with a loop index, on a descending
// range, and against the object task.
class leaf;
  rand int x;
  constraint c { x inside {[1:100000]}; }
endclass
class node;
  rand leaf l[4];
  rand leaf r[3:1];
  rand leaf one;
  function new();
    foreach (l[i]) l[i] = new;
    foreach (r[i]) r[i] = new;
    one = new;
  endfunction
  function int active();
    active = 0;
    for (int i = 0; i < 4; i++) active += this.l[i].rand_mode();
    active += this.one.rand_mode();
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
  node nn;
  int old[4];
  int ok;
  int idx;
  function automatic void snap();
    foreach (old[i]) old[i] = t.n.l[i].x;
  endfunction
  function automatic void show(string what);
    $write("%s: modes", what);
    foreach (old[i]) $write(" %0d", t.n.l[i].rand_mode());
    $write(" changed");
    foreach (old[i]) $write(" %0d", old[i] != t.n.l[i].x);
    $display(" active %0d", t.n.active());
  endfunction
  initial begin
    nn = t.n;
    snap();
    ok = t.randomize();
    show("all");
    t.n.l[1].rand_mode(0);
    t.n.one.rand_mode(0);
    snap();
    ok = t.randomize();
    show("one element off");
    nn.rand_mode(1);
    snap();
    ok = t.randomize();
    show("object on");
    nn.rand_mode(0);
    t.n.l[2].rand_mode(1);
    snap();
    ok = t.randomize();
    show("object off, one element on");
    t.n.l.rand_mode(1);
    t.n.l[0].rand_mode(0);
    t.n.l[0] = new;
    snap();
    ok = t.randomize();
    show("new object in an off element");
    nn.rand_mode(1);
    t.n.rand_mode(0);
    snap();
    ok = t.randomize();
    show("handle of the object off");
    $display("handle: %0d", t.n.rand_mode());
    t.n.rand_mode(1);

    t.n.r[3].rand_mode(0);
    $display("descending: %0d %0d %0d", t.n.r[3].rand_mode(), t.n.r[2].rand_mode(),
             t.n.r[1].rand_mode());
    idx = 0;
    $display("below the range: %0d", t.n.r[idx].rand_mode());
    idx = 4;
    $display("above the range: %0d", t.n.r[idx].rand_mode());
    if (!t.n.l[3].rand_mode()) $display("unexpected");
    else $display("condition: active");
    $finish;
  end
endmodule
