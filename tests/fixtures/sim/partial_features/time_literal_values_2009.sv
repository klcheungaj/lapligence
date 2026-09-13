`timescale 1ns/100ps
module tb;
  parameter real P = 1.54ns;
  real initialized = 1.54ns;
  real value;
  real positive_boundary;
  real negative;
  real arithmetic;

  initial begin
    value = 1.54ns;
    positive_boundary = 1.55ns;
    negative = -1.55ns;
    arithmetic = 1.54ns + 40ps;
    $display("value=%.2f initialized=%.2f positive=%.2f negative=%.2f arithmetic=%.2f parameter=%.2f",
             value, initialized, positive_boundary, negative, arithmetic, P);
    #(1.54ns + 40ps);
    $display("delay=%.2f", $realtime);
    $finish(0);
  end
endmodule
