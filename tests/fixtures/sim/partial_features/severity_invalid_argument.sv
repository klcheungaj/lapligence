// llg-test-fixture: tests/fixtures/sim/partial_features/severity_invalid_argument.sv
module tb;
  initial begin
    $fatal(3, "invalid");
  end
endmodule
