// Static-review counterexample; NOT EXECUTED.
program p;
  initial begin
    fork
      begin #10; $display("ERROR: detached program child survived last initial"); end
    join_none
  end
endprogram
module tb;
  p a();
  initial begin #20; $display("ERROR: programs did not cause implicit finish"); $finish(0); end
  final $display("final");
endmodule
