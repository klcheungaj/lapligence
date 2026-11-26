// Functions with nonblocking assignments of their own as operands of a
// nonblocking assignment to a part of a record (readme: "Operands").
`timescale 1ns / 1ns
package pk;
  typedef struct {
    int a;
    string s;
    real r;
  } rec_t;
  rec_t shared;
endpackage
module tb;
  import pk::*;
  typedef struct packed {
    logic [7:0] hi, lo;
  } pair_t;
  typedef struct {
    rec_t in;
    int arr[3];
    string names[2];
    int q[$];
    pair_t p;
    logic [15:0] v;
  } big_t;
  class item;
    int m;
    static int count;
    static rec_t rec;
    static rec_t recs[2];
  endclass
  rec_t ra[3], one;
  big_t na[3], nn;
  item handle, held[2];
  string sa[3];
  int v, w, x;
  function int f(int k);
    v <= k;
    return k + 1;
  endfunction
  function string g(int k);
    w <= k;
    return $sformatf("s%0d", k);
  endfunction
  function real h(int k);
    v <= k;
    return k + 0.5;
  endfunction
  function int ix(int k);
    x <= 100 + k;
    return k;
  endfunction
  // The function itself writes parts of records.
  function int fr(int k);
    one.s <= $sformatf("inner%0d", k);
    ra[2].a <= 10 * k;
    return k;
  endfunction
  function automatic item make(int k);
    item made = new;
    v <= k;
    made.m = k;
    return made;
  endfunction
  function rec_t whole(int k);
    v <= k;
    whole.a = k;
    whole.s = g(k);
    whole.r = k;
  endfunction
  task show(string step);
    #1;
    $display("%s: v %0d w %0d x %0d", step, v, w, x);
  endtask
  initial begin
    nn.v = 0;
    na[1].v = 0;
  end
  initial begin
    // An element of an array of records.
    ra[1].a <= f(1);
    show("element member");
    ra[1].s <= g(2);
    show("element string");
    ra[ix(0)].r <= h(3);
    show("element real");
    ra[ix(1)] <= '{f(4), g(5), 1.5};
    show("element");
    ra[ix(g(0).len() - 1)].a <= f(6);
    show("nested operands");
    $display("%0d %s %0.1f | %0d %s %0.1f | %0d %s %0.1f", ra[0].a, ra[0].s, ra[0].r, ra[1].a,
             ra[1].s, ra[1].r, ra[2].a, ra[2].s, ra[2].r);
    // Parts of an element.
    na[1].in.a <= f(7);
    na[1].in.s <= g(8);
    nn.p <= f(9);
    na[1].v[7:4] <= f(1);
    na[1].v[ix(12)] <= f(0);
    na[1].arr[ix(2)] <= f(10);
    na[1].names[ix(1)] <= g(11);
    na[1].q <= '{f(12), f(13)};
    show("element parts");
    na[2].in <= whole(14);
    show("element record");
    $display("%0d %s %h %h %0d %s %0d %0d %0d %s", na[1].in.a, na[1].in.s, nn.p, na[1].v,
             na[1].arr[2], na[1].names[1], na[1].q.size(), na[1].q[1], na[2].in.a, na[2].in.s);
    // A record variable, a package record and static properties.
    one.a <= f(15);
    one.s <= g(16);
    nn.arr[ix(1)] <= f(17);
    nn.names[ix(0)] <= g(18);
    nn.v[3:0] <= f(19);
    nn.q <= '{f(20)};
    shared.s <= g(21);
    item::rec.s <= g(22);
    item::recs[ix(1)].a <= f(23);
    item::count <= f(24);
    sa[ix(2)] <= g(25);
    handle <= make(26);
    held[ix(1)] <= make(27);
    show("variables");
    $display("%0d %s %0d %s %h %0d %s %s %0d %0d %s %0d %0d", one.a, one.s, nn.arr[1], nn.names[0],
             nn.v, nn.q[0], shared.s, item::rec.s, item::recs[1].a, item::count, sa[2], handle.m,
             held[1].m);
    // The operand writes parts of records too.
    ra[1].a <= fr(28);
    one.r <= fr(29);
    na[0].names[0] <= $sformatf("outer%0d", fr(30));
    show("inner parts");
    $display("%0d %0d %s %0.1f %s", ra[1].a, ra[2].a, one.s, one.r, na[0].names[0]);
    // Delays.
    ra[0].s <= #(f(2)) g(31);
    show("delay");
    #3 $display("%s", ra[0].s);
    nn.arr[ix(2)] <= #(f(1)) fr(32);
    show("inner delay");
    #2 $display("%0d %0d", nn.arr[2], ra[2].a);
    // An operand replaces the record the write selects from.
    ra[1].s <= replace(33);
    na[1].v[ix(2)+:4] <= #(clear(1)) 4'ha;
    show("replaced");
    #1 $display("%0d %s %h %s", ra[1].a, ra[1].s, na[1].v, na[1].in.s);
    $finish;
  end
  function string replace(int k);
    ra[1] = '{k, "replaced", 0.0};
    return "kept";
  endfunction
  function int clear(int k);
    big_t empty;
    empty.in.s = "empty";
    empty.v = 0;
    na[1] = empty;
    return k;
  endfunction
endmodule
