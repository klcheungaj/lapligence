// SIM-023: a force right-hand side that calls a function ending the run has no
// evaluator callback; the error is located at the `force` statement.
module tb;
  int x;
  function int last(int value);
    if (value == 2) $finish;
    return value;
  endfunction
  initial begin
    x = 1;
    force x = last(2);
  end
endmodule
