// SIM-032 A02: $exit in a module task follows the calling thread: ignored
// for a module thread, terminating the program for a program thread
// (IEEE 1800-2009 24.3.1, 24.7).
module tb;
  task automatic quit;
    $display("quit t=%0d", $time);
    $exit;
  endtask
  p p0();
  q q0();
  initial begin
    quit();
    $display("module continues after ignored exit");
  end
  final $display("final t=%0d", $time);
endmodule

program p;
  initial begin
    #2 tb.quit();
    $display("p after quit must not print");
  end
endprogram

program q;
  initial #4 $display("q alive t=%0d", $time);
endprogram
