// SIM-028 A02: class-object random streams (IEEE 1800-2009 18.13.3-18.13.5,
// 18.14.1, 18.14.3, 18.15; 8.11 for shallow copies). Only relations are
// checked: the state string and the generator are implementation dependent
// (18.13.4).
class packet_c;
  int id;
  // Self-seeding in new, as in the 18.15 example.
  function new(int seed);
    if (seed != 0) this.srandom(seed);
  endfunction
  function string state();
    return get_randstate();
  endfunction
  function void reseed(int seed);
    srandom(seed);
  endfunction
endclass

// A class without properties still has a stream (18.14.1: every object).
class empty_c;
endclass

class derived_c extends packet_c;
  function new();
    super.new(0);
  endfunction
endclass

module tb;
  packet_c a, b, copy;
  packet_c pool[2];
  empty_c e1, e2;
  derived_c d;
  packet_c base;
  string seven, saved;
  int unsigned x, y, z;
  logic [31:0] unknown_seed;
  logic [63:0] wide_seed;

  initial begin
    a = new(7);
    b = new(7);
    seven = a.get_randstate();
    $display("same seed same state %0d", b.get_randstate() == seven);
    b.srandom(8);
    $display("other seed other state %0d", b.get_randstate() != seven);

    saved = a.get_randstate();
    a.srandom(99);
    $display("reseeded %0d", a.get_randstate() != saved);
    a.set_randstate(saved);
    $display("restored %0d", a.get_randstate() == saved);

    // Draws of the thread, seeding of other objects and of the thread do
    // not touch an object's stream.
    x = $urandom;
    b.srandom(5);
    process::self().srandom(3);
    $display("independent of thread and objects %0d", a.get_randstate() == seven);

    // Creation seeds from the creating thread (hierarchical object seeding).
    process::self().srandom(11);
    e1 = new;
    process::self().srandom(11);
    e2 = new;
    $display("seeded from thread %0d", e1.get_randstate() == e2.get_randstate());
    process::self().srandom(11);
    e1 = new;
    e2 = new;
    $display("siblings differ %0d", e1.get_randstate() != e2.get_randstate());

    // Creation takes exactly the thread's next value.
    process::self().srandom(11);
    x = $urandom;
    y = $urandom;
    process::self().srandom(11);
    e1 = new;
    z = $urandom;
    $display("creation takes one value %0d", z == y);

    // Inherited built-ins act on the object, whatever the handle type.
    d = new;
    base = d;
    base.srandom(7);
    $display("base handle %0d", d.get_randstate() == seven);
    d.reseed(8);
    b.srandom(8);
    $display("implicit this %0d", d.state() == b.get_randstate());

    // Element and copied states.
    pool[0] = new(0);
    pool[1] = new(0);
    pool[0].srandom(7);
    pool[1].set_randstate(pool[0].get_randstate());
    $display("element states %0d %0d", pool[0].get_randstate() == seven,
             pool[1].get_randstate() == seven);

    // `int seed`: X/Z bits read as 0, reals round, wide values truncate
    // (6.11.2, 6.24.1).
    unknown_seed = 'x;
    pool[0].srandom(unknown_seed);
    pool[1].srandom(0);
    $display("unknown seed is zero %0d", pool[0].get_randstate() == pool[1].get_randstate());
    pool[0].srandom(6.6);
    $display("real seed rounds %0d", pool[0].get_randstate() == seven);
    wide_seed = 64'h1_0000_0007;
    pool[0].srandom(wide_seed);
    $display("wide seed truncates %0d", pool[0].get_randstate() == seven);

    // A shallow copy copies the RNG state (8.11) and takes no thread value
    // (S28-D6).
    process::self().srandom(11);
    x = $urandom;
    process::self().srandom(11);
    copy = new a;
    y = $urandom;
    $display("copy keeps state %0d %0d", copy.get_randstate() == seven, x == y);
    copy.srandom(8);
    $display("copy independent %0d", a.get_randstate() == seven);
    $finish;
  end
endmodule
