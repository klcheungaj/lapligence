// SIM-026 negative: a string argument is a string or integral value.
module tb;
  real value = 1.5;
  integer result;
  initial begin
    result = $test$plusargs(value);
    $finish;
  end
endmodule
