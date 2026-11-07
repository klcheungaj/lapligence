// S30-D16: `rand_mode` called on a class handle property, or on one element of
// an array of class handle properties, sets the mode of that variable, not of
// the object it refers to. The object stays randomizable on its own; a new
// object in the element keeps the element's mode. A property that is not `rand`
// has no mode and the call has no effect. A handle held anywhere else (a
// variable of a procedure or module) is the object form and changes every
// member of the object.
// 18.8: "The object is any expression that yields the object handle in which
// the random variable is defined." and "If the random variable is an object
// handle, only the mode of the variable is changed, not the mode of random
// variables within that object (see global constraints in 18.5.8)."
// A selector outside the array selects no element and has no effect.
class item;
  rand int x;
  constraint c { x inside {[1:100000]}; }
endclass
class owner;
  rand item h;
  item n;
  item b[3];
  rand item a[3];
  rand item r[3:1];
  function new();
    h = new;
    n = new;
    foreach (b[i]) b[i] = new;
    foreach (a[i]) a[i] = new;
    foreach (r[i]) r[i] = new;
  endfunction
endclass
module tb;
  owner c = new;
  item m[3];
  int old;
  int ok;
  int idx;
  item t;
  initial begin
    foreach (m[i]) begin
      m[i] = new;
      ok = m[i].randomize();
    end
    ok = c.randomize();
    foreach (c.b[i]) ok = c.b[i].randomize();
    ok = c.n.randomize();

    old = c.h.x;
    c.h.rand_mode(0);
    ok = c.randomize();
    $display("scalar off: kept %0d", old == c.h.x);
    ok = c.h.randomize();
    $display("scalar off, own randomize: changed %0d", old != c.h.x);
    c.h.rand_mode(1);
    old = c.h.x;
    ok = c.randomize();
    $display("scalar on: changed %0d", old != c.h.x);

    old = c.a[1].x;
    c.a[1].rand_mode(0);
    ok = c.randomize();
    $display("element off: kept %0d", old == c.a[1].x);
    ok = c.a[1].randomize();
    $display("element off, own randomize: changed %0d", old != c.a[1].x);
    c.a[1] = new;
    old = c.a[1].x;
    ok = c.randomize();
    $display("new object in element: kept %0d", old == c.a[1].x);
    c.a[1].rand_mode(1);
    ok = c.a[1].randomize();
    old = c.a[1].x;
    ok = c.randomize();
    $display("element on: changed %0d", old != c.a[1].x);

    old = c.b[1].x;
    c.b[1].rand_mode(0);
    ok = c.b[1].randomize();
    $display("not rand, element: changed %0d", old != c.b[1].x);
    old = c.n.x;
    c.n.rand_mode(0);
    ok = c.n.randomize();
    $display("not rand, scalar: changed %0d", old != c.n.x);

    old = m[2].x;
    m[2].rand_mode(0);
    ok = m[2].randomize();
    $display("module element: kept %0d", old == m[2].x);
    t = c.a[0];
    old = t.x;
    t.rand_mode(0);
    ok = c.randomize();
    $display("procedure variable: kept %0d", old == c.a[0].x);
    t.rand_mode(1);

    idx = 3;
    old = c.r[3].x;
    c.r[idx].rand_mode(0);
    ok = c.randomize();
    $display("descending range: kept %0d", old == c.r[3].x);
    old = c.r[1].x;
    ok = c.randomize();
    $display("other element: changed %0d", old != c.r[1].x);
    c.r[idx].rand_mode(1);

    c.a.rand_mode(0);
    c.a[1].rand_mode(1);
    old = c.a[0].x;
    idx = c.a[1].x;
    ok = c.randomize();
    $display("whole off, one on: kept %0d changed %0d", old == c.a[0].x, idx != c.a[1].x);
    c.a.rand_mode(1);
    idx = 7;
    c.a[idx].rand_mode(0);
    old = c.a[0].x;
    ok = c.randomize();
    $display("outside the array: %0d changed %0d", ok, old != c.a[0].x);
    $finish;
  end
endmodule
