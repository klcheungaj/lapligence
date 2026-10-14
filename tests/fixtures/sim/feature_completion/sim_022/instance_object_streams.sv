// SIM-022-A01 composition with SV 18.14.1: an object that a static
// initializer constructs seeds from its declaring instance's initialization
// RNG, so identical instances get different object streams, two objects of
// one instance differ, and each instance's objects are independent of the
// other instances' declarations.
class Item;
  int k;
endclass
module child;
  Item a = new;
  Item b = new;
endmodule
module tb;
  child u1();
  child u2();
  Item x = new;
  Item y = new;
  initial begin
    $display("instances differ %0d", u1.a.get_randstate() != u2.a.get_randstate());
    $display("objects of one instance differ %0d", u1.a.get_randstate() != u1.b.get_randstate());
    $display("top objects differ %0d", x.get_randstate() != y.get_randstate());
    $finish;
  end
endmodule
