// llg-test-fixture: tests/fixtures/sim/partial_features/deferred_assertions.sv
module tb;
  logic live;

  task automatic report(input logic sampled, ref logic reference);
    $display("sampled=%0d reference=%0d", sampled, reference);
  endtask

  initial begin
    live = 1'b0;
    assert #0 (1'b0) else report(live, live);
    live = 1'b1;
    #1;
    $finish(2);
  end
endmodule
