// S30-D17: the function form of `rand_mode` on a `rand` class handle property,
// or on one element of a fixed array of `rand` class handle properties, returns
// the mode of that variable: the array slot, not the object the slot refers to.
// `c.a[1].rand_mode(0)` therefore turns off slot 1 of `a` only: the random
// variables of the object keep their own modes, `c.randomize()` leaves the
// object alone, and the object still randomizes through another handle.
// The task form on the whole object sets every slot too, and a selector outside
// the array names no slot, which reads as active.
// 18.8: "function int object.random_variable::rand_mode();", "When called as a
// function, rand_mode() returns the current active state of the specified
// random variable. It returns 1 if the variable is active (ON) and 0 if the
// variable is inactive (OFF)." and "If the random variable is an object handle,
// only the mode of the variable is changed, not the mode of random variables
// within that object (see global constraints in 18.5.8)."
// The function form on a handle that is not a random variable (`e.rand_mode()`
// of a module variable) names no random variable and is a compile error.
class E;
  rand int v;
endclass
class C;
  rand E a[3];
  rand E h;
  function new;
    foreach (a[i]) begin
      a[i] = new;
      a[i].v = 0;
    end
    h = new;
  endfunction
endclass
module tb;
  C c = new;
  E e;
  int changed1 = 0, changed_e = 0, v1, idx;
  initial begin
    c.a[1].rand_mode(0);
    $display("slot modes a0 %0d a1 %0d a2 %0d", c.a[0].rand_mode(), c.a[1].rand_mode(),
             c.a[2].rand_mode());
    $display("object variable mode a1.v %0d", c.a[1].v.rand_mode());
    repeat (10) begin
      v1 = c.a[1].v;
      if (!c.randomize()) $display("c fail");
      if (c.a[1].v != v1) changed1++;
    end
    $display("c.randomize changed a[1].v %0d of 10 times", changed1);
    e = c.a[1];
    repeat (10) begin
      v1 = e.v;
      if (!e.randomize()) $display("e fail");
      if (e.v != v1) changed_e++;
    end
    $display("e.randomize changed e.v %0d of 10 times", changed_e);

    idx = 5;
    $display("outside the array %0d", c.a[idx].rand_mode());
    $display("handle property %0d", c.h.rand_mode());
    c.h.rand_mode(0);
    $display("handle property off %0d, its variable %0d", c.h.rand_mode(), c.h.v.rand_mode());
    c.rand_mode(1);
    $display("object on: %0d %0d %0d h %0d", c.a[0].rand_mode(), c.a[1].rand_mode(),
             c.a[2].rand_mode(), c.h.rand_mode());
    c.rand_mode(0);
    $display("object off: %0d %0d %0d h %0d", c.a[0].rand_mode(), c.a[1].rand_mode(),
             c.a[2].rand_mode(), c.h.rand_mode());
    c.a[2].rand_mode(1);
    $display("one slot on: %0d %0d %0d", c.a[0].rand_mode(), c.a[1].rand_mode(),
             c.a[2].rand_mode());
    c.a.rand_mode(1);
    $display("array on: %0d %0d %0d", c.a[0].rand_mode(), c.a[1].rand_mode(),
             c.a[2].rand_mode());
    $display("done");
    $finish;
  end
endmodule
