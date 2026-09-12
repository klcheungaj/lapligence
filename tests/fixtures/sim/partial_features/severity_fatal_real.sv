// llg-test-fixture: tests/fixtures/sim/partial_features/severity_fatal_real.sv
module tb;
  real value = 1.25;

  initial begin
    $fatal(value);
    $display("stdout after");
  end

  final begin
    $display("stdout final");
  end
endmodule
