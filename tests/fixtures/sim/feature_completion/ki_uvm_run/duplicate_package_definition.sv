`timescale 1ns/1ns
`include "duplicate_package_definition_pkg.svh"
// With separate compilation units the header's guard does not carry over
// from the file that compiled the package first, so the package is defined
// twice; every reference names the first definition.
module tb;
  import dup_pkg::*;
  initial begin
    int unsigned a, b, c;
    a = next_seed("top.a");
    b = next_seed("top.a");
    c = next_seed("top.b");
    $display("a=%0d b=%0d c=%0d created=%0d entries=%0d", a, b, c, created, table_lookup.num());
    $finish;
  end
endmodule
