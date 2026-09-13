// llg-test-fixture: tests/fixtures/sim/partial_features/time_literal_exact_2009.sv
module tb;
  timeunit 1ns;
  timeprecision 1fs;

  real positive;
  real negative;
  real scientific;

  initial begin
    positive = 1.0000005ns;
    negative = -1.0000005ns;
    scientific = 0.000001ns;
    #(1.0000005ns);
    $display("exact=%.6f %.6f %.6f delay=%.6f", positive, negative, scientific, $realtime);
    $finish(0);
  end
endmodule
