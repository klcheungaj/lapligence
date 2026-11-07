// llg-test-fixture: tests/fixtures/sim/partial_features/severity_fatal_in_fork.sv
module tb;
  task automatic stop_later;
    #1 $fatal(1, "late fatal");
  endtask

  initial begin
    fork
      stop_later();
      #2 $display("stdout after the fatal");
    join_none
    #3 $display("stdout never");
  end

  final begin
    $display("stdout final");
  end
endmodule
