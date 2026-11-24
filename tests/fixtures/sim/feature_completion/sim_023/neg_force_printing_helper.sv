// SIM-023: a force right-hand side that calls a printing function has no
// evaluator callback; the error is located at the `force` statement.
module tb;
  int x;
  function int show(int value);
    $display("show %0d", value);
    return value;
  endfunction
  initial begin
    x = 1;
    force x = show(2);
  end
endmodule
