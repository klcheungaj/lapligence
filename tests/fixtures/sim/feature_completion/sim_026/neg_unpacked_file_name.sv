// SIM-026 negative: a string argument is a string or integral value.
module tb;
  reg [7:0] value [0:1];
  integer result;
  initial begin
    result = $fopen(value, "w");
    $finish;
  end
endmodule
