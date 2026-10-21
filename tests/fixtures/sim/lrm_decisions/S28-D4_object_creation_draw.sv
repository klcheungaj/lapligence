// IEEE 1800-2009 18.14.1 L31161-31163: "Object stability. Each class
// instance (object) has an independent RNG for all randomization methods in
// the class. When an object is created using new, its RNG is seeded with the
// next random value from the thread that creates the object."
// 18.13.3 L31089: "The srandom() method initializes an object's RNG using
// the value of the given seed."
// Decision (LRM text; that the consumed value is the one the thread's next
// $urandom would return is llg's reading): `new` takes exactly one value from
// the creating thread, so objects created from equal thread states have
// equal states, and the thread continues as if one $urandom had run.
// The object states are compared through the values randomize() draws from
// them; the form of get_randstate() strings is not specified.
class item_c;
  rand int unsigned v;
endclass

module tb;
  item_c a, b;
  int unsigned x, y, z;
  initial begin
    process::self().srandom(11);
    x = $urandom;
    y = $urandom;
    process::self().srandom(11);
    a = new;
    z = $urandom;
    $display("new takes one value %0d", z == y);
    process::self().srandom(11);
    b = new;
    void'(a.randomize());
    void'(b.randomize());
    $display("equal thread states %0d", a.v == b.v);
    process::self().srandom(11);
    a = new;
    b = new;
    void'(a.randomize());
    void'(b.randomize());
    $display("next object differs %0d", a.v != b.v);
    a.srandom(4);
    b.srandom(4);
    void'(a.randomize());
    void'(b.randomize());
    $display("equal seeds %0d", a.v == b.v);
    $finish;
  end
endmodule
