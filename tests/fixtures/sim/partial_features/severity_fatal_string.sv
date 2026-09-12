// llg-test-fixture: tests/fixtures/sim/partial_features/severity_fatal_string.sv
module tb;
  string message;

  initial begin
    message = "dynamic fatal";
    $fatal(message);
    $display("stdout after");
  end

  final begin
    $display("stdout final");
  end
endmodule
