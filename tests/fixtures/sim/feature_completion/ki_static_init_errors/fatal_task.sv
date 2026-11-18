// $fatal in a function a package variable's declaration initializer calls:
// the run ends at the call; neither the rest of the function, the package's
// later initializer, the module's initializers nor any procedure runs.
// Expected values are derived in readme.md.
package p;
  function automatic int stop(int value);
    $display("stop called");
    $fatal(1, "package initializer fatal");
    $display("after fatal");
    return value;
  endfunction
  int pv = stop(3);
  int pw = 4;
endpackage
module tb;
  function automatic int note(string s);
    $display("init %s", s);
    return 1;
  endfunction
  int first = note("first");
  initial begin
    $display("initial first=%0d pv=%0d pw=%0d", first, p::pv, p::pw);
    $finish;
  end
endmodule
