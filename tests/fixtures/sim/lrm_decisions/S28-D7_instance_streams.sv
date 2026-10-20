// IEEE 1800-2009 18.14.1 L31148-31156: "Each module instance, interface
// instance, program instance, and package has an initialization RNG. Each
// initialization RNG is seeded with the default seed. The default seed is an
// implementation-dependent value. ... When a static process is created, its
// RNG is seeded with the next value from the initialization RNG of the module
// instance, interface instance, program instance, or package containing the
// thread declaration."
// Decision (clause text): every instance's initialization RNG starts from
// the same default seed, so the first static process of two identical
// instances gets the same seed and draws the same value. Processes added to
// one instance still never move another instance's seeds.
module drawer;
  int unsigned v;
  initial v = $urandom;
endmodule

module tb;
  drawer d1();
  drawer d2();
  initial begin
    #1;
    $display("identical instances draw the same value %0d", d1.v == d2.v);
    $finish;
  end
endmodule
