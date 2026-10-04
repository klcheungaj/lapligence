// IEEE 1800-2009 7.12.2: reverse permutes real elements numerically intact.
// SIM-005 admits real fixed arrays; -0.0 keeps its sign bit.
module tb;
  real values [0:2];
  initial begin
    values[0] = 1.5;
    values[1] = -0.0;
    values[2] = -2.25;
    values.reverse();
    $display("%0.2f %0.2f %0.2f %h", values[0], values[1], values[2],
             $realtobits(values[1]));
    $finish;
  end
endmodule
