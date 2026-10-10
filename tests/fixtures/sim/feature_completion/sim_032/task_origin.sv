// SIM-032 A01: a module task inherits the region set of the thread that
// calls it (IEEE 1800-2009 24.3.1, 24.5).
module tb;
  logic nb = 1'b0;
  int last = 0;
  task automatic T(input int tag);
    $display("T%0d S1 nb=%0d last=%0d t=%0d", tag, nb, last, $time);
    last <= tag;
    #0 $display("T%0d after #0 last=%0d", tag, last);
    #2 $display("T%0d after #2 last=%0d t=%0d", tag, last, $time);
  endtask
  always @(last) $display("module saw last=%0d t=%0d", last, $time);
  initial begin
    #5 nb <= 1'b1;
    T(1);
  end
  program pr;
    initial begin
      #5 T(2);
      $display("program done t=%0d", $time);
    end
  endprogram
endmodule
