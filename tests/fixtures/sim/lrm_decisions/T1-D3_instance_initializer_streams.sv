// IEEE 1800-2009 18.14.1 L31148-31151 (SystemVerilog-1800-2009.txt): "Each
// module instance, interface instance, program instance, and package has an
// initialization RNG. Each initialization RNG is seeded with the default
// seed. The default seed is an implementation-dependent value. An
// initialization RNG shall be used in the creation of static processes and
// static initializers".
// L31164-31166: "When a class object is created by a static declaration
// initializer, there is no active thread; thus, the RNG of the created object
// is seeded with the next random value of the initialization RNG of the
// module instance, interface instance, program instance, or package in which
// the declaration occurred."
// Decision (clause text, values by reference run): static initializers of
// each instance or package draw that instance's own stream, so identical
// instances draw identical values and an initializer added to one instance
// moves no other. Objects created by static initializers draw the same stream.
// The text leaves open whether static processes share that state; they do
// not: an added initializer moves no static process seed.
class holder;
  rand int value;
endclass

package pk;
  int first = $urandom;
endpackage

module drawer;
  function automatic int through_call();
    return $urandom;
  endfunction
  int a = $urandom;
  int b = through_call();
  int c[2] = '{$urandom, $urandom};
  int range = $urandom_range(1000000);
  int spawned;
  initial spawned = $urandom;
endmodule

module grown;
  int a = $urandom;
  int more = $urandom;
  holder made = new;
  int after = $urandom;
  int spawned;
  initial spawned = $urandom;
endmodule

module plain;
  int a = $urandom;
  int after = $urandom;
  int spawned;
  initial spawned = $urandom;
endmodule

module tb;
  drawer u0();
  drawer u1();
  plain p0();
  grown g0();
  plain p1();
  int top_first = $urandom;
  initial begin
    #1;
    $display("identical a %0d", u0.a == u1.a);
    $display("identical b %0d", u0.b == u1.b);
    $display("identical c %0d", u0.c[0] == u1.c[0] && u0.c[1] == u1.c[1]);
    $display("identical range %0d", u0.range == u1.range);
    $display("identical spawned %0d", u0.spawned == u1.spawned);
    $display("a draws differ %0d", u0.a != u0.b);
    $display("first value everywhere %0d", p0.a == g0.a && g0.a == top_first
                                           && top_first == pk::first);
    $display("added initializers leave the next instance %0d", p0.after == p1.after);
    $display("objects and draws share the stream %0d", g0.after != p0.after && g0.more == p0.after);
    $display("added initializers leave process seeds %0d",
             g0.spawned == p0.spawned && p0.spawned == p1.spawned);
    $finish;
  end
endmodule
