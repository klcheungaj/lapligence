// llg-test-fixture: tests/fixtures/sim/partial_features/severity_error_then_fatal.sv
module tb;
  initial begin
    $error("e1");
    $display("stdout between");
    $fatal(1, "boom");
    $display("stdout after");
  end
endmodule
