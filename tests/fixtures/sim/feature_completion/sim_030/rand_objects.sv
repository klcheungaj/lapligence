// SIM-030: random object members and global constraints (IEEE 1800-2009
// 18.4, 18.5.8, 18.6.2; decision S30-D7). Each line prints relations every
// legal solution satisfies.
class A;
  rand bit [7:0] v;
endclass

class B extends A;
  rand A left;
  rand A right;
  constraint heapcond { left.v <= v; right.v <= v; }
endclass

class item_c;
  rand bit [3:0] x;
  randc bit [1:0] r;
  int id;
  static bit verbose = 1;
  constraint cx { x > 4'd10; }
  function void pre_randomize(); if (verbose) $display("pre %0d", id); endfunction
  function void post_randomize(); if (verbose) $display("post %0d x_ok=%0d", id, x > 4'd10); endfunction
endclass

class pair_c;
  rand item_c a, b;
  item_c plain;
  rand bit [3:0] t;
  int id;
  constraint ct { a.x == t; b != null -> b.x == t; plain != null -> plain.x != t; }
  function void pre_randomize(); if (item_c::verbose) $display("pre pair"); endfunction
  function void post_randomize(); if (item_c::verbose) $display("post pair a=%0d", a.x == t); endfunction
endclass

class list_c;
  rand bit [3:0] v;
  rand list_c next;
  constraint cn { next != null -> next.v == v + 4'd1; }
endclass

module tb;
  B root, mid;
  A l1, l2, l3;
  pair_c p;
  item_c i1, i2, i3;
  list_c n1, n2, n3;
  int bad, seen;

  initial begin
    // The heap example: every node's children hold smaller or equal values,
    // and the leaves, the inner node and the root are solved together.
    root = new; mid = new; l1 = new; l2 = new; l3 = new;
    root.left = mid; root.right = l1; mid.left = l2; mid.right = l3;
    for (int i = 0; i < 100; i++) begin
      void'(root.randomize());
      if (mid.v > root.v || l1.v > root.v || l2.v > mid.v || l3.v > mid.v) bad = bad + 1;
    end
    $display("heap: bad=%0d", bad);
    // A null rand handle is skipped; a guard keeps its member out of the
    // constraint (18.5.12).
    p = new; i1 = new; i2 = new; i3 = new;
    i1.id = 1; i2.id = 2; i3.id = 3;
    p.a = i1;
    $display("null: ok=%0d same=%0d", p.randomize(), i1.x == p.t);
    // pre_randomize runs on the object, then on its random members as they
    // are reached; post_randomize in the same order (S30-D7). The object of
    // a non-rand handle is state and its constraints do not apply (18.5.8
    // b): i3.x == 2 breaks its own cx, yet the call succeeds.
    p.b = i2; p.plain = i3; i3.x = 4'd2;
    $display("objects: ok=%0d a=%0d b=%0d plain=%0d", p.randomize(), i1.x == p.t, i2.x == p.t,
             i3.x);
    // Inactive variables of a random object are state (18.8).
    i2.rand_mode(0);
    i2.x = 4'd13;
    $display("inactive: ok=%0d t=%0d b=%0d", p.randomize(), p.t, i2.x);
    i2.rand_mode(1);
    // An object reached twice is one object.
    item_c::verbose = 0;
    p.b = i1;
    bad = 0;
    for (int i = 0; i < 20; i++) begin
      void'(p.randomize());
      if (i1.x != p.t) bad = bad + 1;
    end
    $display("shared: bad=%0d", bad);
    // An inline constraint reaches a member object (18.7).
    $display("inline: ok=%0d a=%0d", p.randomize() with { a.x == 4'd14; }, i1.x);
    // randc cycles per object (S30-D3): four calls of an object not
    // randomized before visit every value of r.
    for (int i = 0; i < 4; i++) begin
      void'(i3.randomize());
      seen = seen | (1 << i3.r);
    end
    $display("randc: seen=%b", seen[3:0]);
    // A list solved as a whole; a cycle back to the first node is the same
    // object, so the increments cannot close the loop.
    n1 = new; n2 = new; n3 = new;
    n1.next = n2; n2.next = n3;
    $display("list: ok=%0d step=%0d", n1.randomize(),
             n2.v == n1.v + 4'd1 && n3.v == n2.v + 4'd1);
    n3.next = n1;
    $display("cycle: ok=%0d", n1.randomize());
    $finish;
  end
endmodule
