// SIM-032 A01: two programs read a design value written in the Active region
// (ordered), then race on a shared design variable in one Reactive region
// (IEEE 1800-2009 4.4.2.6: "can be processed in any order").
module tb;
  int d;
  int shared = 0;
  initial d = 5;
  program p1;
    initial begin
      $display("p1 d=%0d", d);
      shared = 1;
    end
  endprogram
  program p2;
    initial begin
      $display("p2 d=%0d", d);
      shared = 2;
    end
  endprogram
  final $display("final shared=%0d", shared);
endmodule
