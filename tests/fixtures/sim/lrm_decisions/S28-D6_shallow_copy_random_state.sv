// IEEE 1800-2009 8.11 L9990-9998: "A shallow copy executes in the following
// manner: 1) An object of the class type being copied is allocated. ...
// 2) All class properties, including the internal states used for
// randomization and coverage are copied to the new object. ... The internal
// states for randomization include the random number generator (RNG)
// state". 18.14.1 L31162-31163: "When an object is created using new, its
// RNG is seeded with the next random value from the thread".
// Decision (LRM text for the copied state; that the copy takes no value from
// the thread is an llg choice, since 8.11 lists no seeding step): `new h`
// copies h's RNG state and leaves the thread's stream unchanged.
class item_c;
  int k;
endclass

module tb;
  item_c a, b;
  int unsigned x, y;
  initial begin
    a = new;
    a.srandom(5);
    process::self().srandom(11);
    x = $urandom;
    process::self().srandom(11);
    b = new a;
    y = $urandom;
    $display("state copied %0d", a.get_randstate() == b.get_randstate());
    $display("thread unchanged %0d", x == y);
    b.srandom(6);
    $display("copy independent %0d", a.get_randstate() != b.get_randstate());
    $finish;
  end
endmodule
