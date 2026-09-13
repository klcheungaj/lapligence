// llg-test-fixture: tests/fixtures/sim/partial_features/deferred_assertions_module.sv
module tb;
  task automatic report(input logic value);
    $display("module=%0d", value);
  endtask

  assert #0 (1'b0) else report(1'b1);

  initial begin
    #0;
    $finish(2);
  end
endmodule
