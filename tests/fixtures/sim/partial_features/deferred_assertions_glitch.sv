// llg-test-fixture: tests/fixtures/sim/partial_features/deferred_assertions_glitch.sv
module tb;
  integer reports;

  task automatic report(input integer sampled);
    reports = reports + 1;
    $display("glitch=%0d", sampled);
  endtask

  initial begin
    reports = 0;
    for (integer i = 0; i < 2; i = i + 1)
      assert #0 (i != 0) else report(i);
    #0;
    $display("reports=%0d", reports);
    $finish(2);
  end
endmodule
