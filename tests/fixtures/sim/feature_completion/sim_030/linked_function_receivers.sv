// Linked structures of one class whose constraints call functions (IEEE
// 1800-2009 18.5.11): every object evaluates its own calls with itself as the
// receiver, also the randomized object reached again through a ring, objects
// of a derived class, and elements of handle arrays and queues of the class.
// Lines print relations every legal solution satisfies.
class node;
  rand bit [3:0] v;
  rand bit [3:0] w;
  rand bit [3:0] d[2];
  bit [3:0] lim;
  rand node next;
  rand node kids[2];
  rand node more[$];
  function bit [3:0] cap(bit [3:0] x);
    return x < lim ? x : lim;
  endfunction
  virtual function bit [3:0] bias(bit [3:0] x);
    return x | 4'd1;
  endfunction
  constraint own { w == cap(v); }
  constraint elem { d[1] == cap(d[0]); foreach (d[i]) d[i] <= bias(v); }
  constraint other { if (next != null) next.d[0] <= next.cap(v); }
  constraint tree {
    foreach (kids[i]) if (kids[i] != null) kids[i].v >= cap(i[3:0] + 4'd1);
    foreach (more[i]) more[i].v >= cap(more[i].lim);
  }
endclass

class wide_node extends node;
  rand bit [3:0] extra;
  virtual function bit [3:0] bias(bit [3:0] x);
    return x | 4'd8;
  endfunction
  constraint derived { extra == cap(w) + 4'd1; }
endclass

module tb;
  node a = new, b = new, c = new, k0 = new, k1 = new, m0 = new;
  wide_node x = new;
  int fails, bad;
  function automatic void check(node n);
    wide_node wn;
    if (n.w != n.cap(n.v) || n.d[1] != n.cap(n.d[0])) bad++;
    foreach (n.d[i]) if (n.d[i] > n.bias(n.v)) bad++;
    if (n.next != null && n.next.d[0] > n.next.cap(n.v)) bad++;
    foreach (n.kids[i]) if (n.kids[i] != null && n.kids[i].v < n.cap(i[3:0] + 4'd1)) bad++;
    foreach (n.more[i]) if (n.more[i].v < n.cap(n.more[i].lim)) bad++;
    if ($cast(wn, n) && wn.extra != wn.cap(wn.w) + 4'd1) bad++;
  endfunction
  function automatic void check_all();
    check(a); check(b); check(c); check(k0); check(k1); check(m0); check(x);
  endfunction
  initial begin
    a.lim = 3; b.lim = 7; c.lim = 1; k0.lim = 9; k1.lim = 12; m0.lim = 5; x.lim = 6;
    // a -> b -> c -> a: a ring through the randomized object.
    a.next = b; b.next = c; c.next = a;
    // b has two children, one pointing back at b, and a queue holding an
    // object of the derived class and a plain one.
    b.kids[0] = k0; b.kids[1] = k1; k1.next = b;
    b.more.push_back(x); b.more.push_back(m0);
    // The derived object points into the ring too.
    x.next = c;
    repeat (200) begin
      if (!a.randomize()) fails++;
      check_all();
    end
    $display("from the ring's first object: fails=%0d bad=%0d", fails, bad);
    repeat (200) begin
      if (!b.randomize()) fails++;
      check_all();
    end
    $display("from the object with children: fails=%0d bad=%0d", fails, bad);
    repeat (200) begin
      if (!x.randomize()) fails++;
      check_all();
    end
    $display("from the derived object: fails=%0d bad=%0d", fails, bad);
    repeat (200) begin
      if (!k1.randomize() with { v == cap(4'd10); }) fails++;
      check_all();
      if (k1.v != 4'd10) bad++;
    end
    $display("inline call on a child: fails=%0d bad=%0d", fails, bad);
    $finish;
  end
endmodule
