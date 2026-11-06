// Decision S30-D13: a handle array randomize() grows has null new elements.
//
// IEEE 1800-2009 18.4 (SystemVerilog-1800-2009.txt L29364-29367):
//   "Randomize does not allocate any class objects. Up to the new size,
//   existing class objects are retained and their content randomized. If the
//   new size is greater than the original size, each of the additional
//   elements has a null value requiring no randomization."
//
// A constraint guarded by a null test of a new element is therefore met and
// the call returns 1. Some simulators fail such a call and leave the array as
// it was; llg follows the clause text, which leaves nothing to fail.
class leaf_c;
  rand bit [3:0] v;
endclass

class c_c;
  rand leaf_c a[];
  constraint e { foreach (a[i]) a[i] != null -> a[i].v == 7; }
endclass

module tb;
  initial begin
    static c_c c = new;
    int ok;
    c.a = new[1];
    c.a[0] = new;
    ok = c.randomize() with { a.size() == 3; };
    $display("ok=%0d size=%0d first=%0d null=%0d %0d", ok, c.a.size(), c.a[0].v, c.a[1] == null,
             c.a[2] == null);
    $finish(0);
  end
endmodule
