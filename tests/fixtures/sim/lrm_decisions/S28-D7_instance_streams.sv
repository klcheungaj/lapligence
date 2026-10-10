// IEEE 1800-2009 18.14.1 L31148-31156: "Each module instance, interface
// instance, program instance, and package has an initialization RNG. Each
// initialization RNG is seeded with the default seed. The default seed is an
// implementation-dependent value. ... When a static process is created, its
// RNG is seeded with the next value from the initialization RNG of the module
// instance, interface instance, program instance, or package containing the
// thread declaration."
// Decision (llg choice; the default seed is implementation dependent): each
// instance's initialization RNG derives from the default seed and the
// instance's hierarchical name, so identical instances draw different
// values, and processes added to one instance never move another's seeds.
module drawer;
  int unsigned v;
  initial v = $urandom;
endmodule

module tb;
  drawer d1();
  drawer d2();
  initial begin
    #1;
    $display("identical instances differ %0d", d1.v != d2.v);
    $finish;
  end
endmodule
