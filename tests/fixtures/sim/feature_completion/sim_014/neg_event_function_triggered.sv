// The triggered property names an event identifier (SV 15.5.3), not the
// result of a function returning an event. See the readme.
`timescale 1ns/1ns
module tb;
  function automatic event pass(event f);
    return f;
  endfunction
  event a;
  initial begin
    fork
      wait (pass(a).triggered);
      #1 -> a;
    join
    $finish;
  end
endmodule
