// llg-test-fixture: tests/fixtures/sim/partial_features/severity_no_args.sv
module tb;
  initial begin
    $info;
    $warning;
    $error;
    $display("stdout after");
    $finish(0);
  end
endmodule
