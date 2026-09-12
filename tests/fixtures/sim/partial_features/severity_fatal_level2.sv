// llg-test-fixture: tests/fixtures/sim/partial_features/severity_fatal_level2.sv
module tb;
  initial begin
    $fatal(2, "fatal level 2");
    $display("stdout after");
  end

  final begin
    $display("stdout final");
  end
endmodule
