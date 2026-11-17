`timescale 1ns/1ns
// $error in functions that a class static property's and a module variable's
// declaration initializers call: each reports and initialization and the run
// go on. Expected values are derived in readme.md.
class counter;
  static int base = check("class static", 5);
  static function int check(string what, int value);
    $error("%s initializer error", what);
    return value;
  endfunction
endclass
module tb;
  function automatic int note(string s);
    $display("init %s", s);
    return 1;
  endfunction
  int first = note("first");
  int v = counter::check("module", 7);
  int last = note("last");
  initial begin
    $display("initial first=%0d v=%0d last=%0d base=%0d", first, v, last, counter::base);
    #1 $display("at %0d", $time);
    $finish;
  end
endmodule
