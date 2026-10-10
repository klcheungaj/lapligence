// IEEE 1800-2009 11.11: `+`, `==` and `=` overloads on an unpacked record
// with string, queue, dynamic-array and class-handle members. Every result is
// an independently owned value: changing it never changes an operand, and
// changing an operand never changes an earlier result. `=` overloads convert
// a string and an int in assignments, subroutine arguments and a queue
// method argument.
module tb;
  class C;
    int v;
    function new(int x);
      v = x;
    endfunction
  endclass
  typedef struct { string s; int q[$]; int d[]; C h; } bag_t;

  function automatic bag_t badd(bag_t a, bag_t b);
    badd.s = {a.s, b.s};
    badd.q = {a.q, b.q};
    badd.d = new[a.d.size() + b.d.size()];
    foreach (a.d[i]) badd.d[i] = a.d[i];
    foreach (b.d[i]) badd.d[a.d.size() + i] = b.d[i];
    badd.h = new(a.h.v + b.h.v);
  endfunction
  function automatic bag_t bfroms(string s);
    bfroms.s = s;
    bfroms.q = '{s.len()};
    bfroms.d = new[1];
    bfroms.d[0] = s.len();
    bfroms.h = new(s.len());
  endfunction
  function automatic bag_t bfromi(int i);
    bfromi.s = $sformatf("<%0d>", i);
    bfromi.q = '{i};
    bfromi.d = new[0];
    bfromi.h = new(i);
  endfunction
  function automatic bit beq(bag_t a, string s);
    return a.s == s;
  endfunction

  bind + function bag_t badd(bag_t, bag_t);
  bind = function bag_t bfroms(string);
  bind = function bag_t bfromi(int);
  bind == function bit beq(bag_t, string);

  function automatic void show(string tag, bag_t b);
    $display("%s %s %p %p %0d", tag, b.s, b.q, b.d, b.h.v);
  endfunction

  bag_t x, y, z, list[$];
  string word, upper;

  initial begin
    word = "ab";
    x = word;
    y = 5;
    show("x", x);
    show("y", y);
    z = x + y;
    z.s = {z.s, "!"};
    z.q.push_back(9);
    z.d[0] = 99;
    z.h.v = 70;
    show("z", z);
    show("x", x);
    show("y", y);
    x.s = "AB";
    x.q[0] = 20;
    show("z", z);
    z = x + y + x;
    show("zz", z);
    list.push_back(x + y);
    list.push_back(word);
    show("l0", list[0]);
    show("l1", list[1]);
    show("arg", x + y);
    show("conv", word);
    upper = "AB";
    $display("eq %0d %0d %0d", z == upper, x == upper, x == word);
    $finish;
  end
endmodule
