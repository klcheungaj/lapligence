// SIM-020 A03: a queue of real is not a bit-stream type.
module tb;
  real rq[$];
  byte q[$];
  initial begin
    q = {>>{rq}};
    $finish;
  end
endmodule
