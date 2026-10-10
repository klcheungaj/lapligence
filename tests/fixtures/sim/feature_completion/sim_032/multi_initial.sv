// SIM-032 A02: a program ends when all of its initials end; that terminates
// its remaining descendants (IEEE 1800-2009 24.3).
program p;
  int n = 0;
  initial begin
    #2 n++;
    $display("i1 t=%0d", $time);
  end
  initial begin
    fork
      begin
        #10 $display("i2 child must not print");
      end
    join_none
    #4 n++;
    $display("i2 t=%0d", $time);
  end
  final $display("program final n=%0d t=%0d", n, $time);
endprogram

module tb;
  p p0();
  initial forever #3 $display("tick t=%0d", $time);
endmodule
