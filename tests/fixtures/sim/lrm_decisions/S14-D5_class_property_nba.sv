// Decision S14-D5: a nonblocking assignment to a class property is a
// compile error.
//
// IEEE 1800-2009 6.21 (SystemVerilog-1800-2009.txt L7007-7008): "Automatic
//   variables and members or elements of dynamic variables—class properties
//   and dynamically sized variables—shall not be written with nonblocking,
//   continuous, or procedural continuous assignments."
//
// llg follows the 1800-2009 text. (IEEE 1800-2012 relaxed this rule for
// class properties; a simulator applying the later rule prints "x=5".)
// Expected result: the design is rejected at compile time; it prints nothing
// (the .out file is empty).
module tb;
  class C;
    int x;
  endclass
  C h;
  initial h = new;
  initial #1 h.x <= 5;
  initial #2 begin
    $display("x=%0d", h.x);
    $finish;
  end
endmodule
