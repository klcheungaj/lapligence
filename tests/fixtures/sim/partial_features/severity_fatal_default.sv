// llg-test-fixture: tests/fixtures/sim/partial_features/severity_fatal_default.sv
module tb;
  initial begin
    $fatal("fatal default");
    $display("stdout after");
  end

  final begin
    $display("stdout final");
  end
endmodule
