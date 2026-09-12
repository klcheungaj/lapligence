// llg-test-fixture: tests/fixtures/sim/partial_features/severity_runtime_argument.sv
module tb;
  integer finish_number;

  initial begin
    finish_number = 1;
    $fatal(finish_number, "runtime");
  end
endmodule
