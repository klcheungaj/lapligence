// IEEE 1800-2009 18.6.2 L30445-30447: "When obj.randomize() is invoked, it
// first invokes pre_randomize() on obj and also all of its random object
// members that are enabled. After the new random values are computed and
// assigned, randomize() invokes post_randomize() on obj and also all of its
// random object members that are enabled." The order among the objects is
// not specified.
// Decision S30-D7 (llg choice): the randomized object's hook runs first, then
// the members' in breadth-first order of their rand handles (declaration
// order within a class, base class properties first); an object reached
// twice runs its hooks once. post_randomize follows the same order, after
// every value is committed.
class node_c;
  string name;
  rand node_c a, b;
  rand bit [3:0] v;
  function new(string n); name = n; endfunction
  function void pre_randomize(); $display("pre %s", name); endfunction
  function void post_randomize(); $display("post %s", name); endfunction
endclass

module tb;
  node_c root, x, y, z;
  initial begin
    root = new("root"); x = new("x"); y = new("y"); z = new("z");
    root.a = x; root.b = y; x.a = z; y.a = z;
    void'(root.randomize());
    $finish;
  end
endmodule
