// SIM-026 negative: a string argument is a string or integral value.
module tb;
  real value = 1.5;
  integer result;
  initial begin
    result = $fopen(value, "w");
    $finish;
  end
endmodule
