// SIM-022-A02: the second file declares compilation-unit variables with the
// first file's names. Separate units keep two independent sets; one merged
// unit would declare each name twice.
string unit_text = "unitB";
int unit_queue[$] = '{7, 8};
module tb;
  ma a();
  initial begin
    #1;
    $display("tb text=%s queue=%0d", unit_text, unit_queue.size());
    $finish;
  end
endmodule
