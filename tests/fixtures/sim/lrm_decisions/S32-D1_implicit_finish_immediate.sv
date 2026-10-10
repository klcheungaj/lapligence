// Decision S32-D1: the implicit $finish that ends simulation when every
// program initial thread has ended is called at that moment, like an explicit
// $finish executed by the last ending thread: events still pending in the
// same time slot (here a Re-NBA update and the design process it would wake)
// do not execute; final procedures then observe the last committed values.
//
// IEEE 1800-2009 24.3 (SystemVerilog-1800-2009.txt L43229-43232):
//   "If there is at least one initial procedure within at least one program
//   block, the entire simulation shall terminate by means of an implicit
//   call to the $finish system task immediately after all the threads and
//   all their descendent threads originating from all initial procedures
//   within all programs have ended."
// IEEE 1800-2009 9.2.3 (L11258-11261):
//   "No remaining scheduled events shall execute after all final procedures
//   have executed. A final procedure executes when simulation ends due to an
//   explicit or implicit call to $finish."
//
// The text does not say whether pending same-slot events run before the
// implicit call; llg treats "immediately" as no further event. The program
// ends at time 0, so the module's #1 display never runs either; $finish is
// implicit (the case deliberately has no explicit $finish).
module tb;
  logic v = 1'b0;
  always @(v) $display("module saw v=%0d", v);
  initial #1 $display("module time 1 must not print");
  program p;
    initial begin
      v <= 1'b1;
      $display("program ends v=%0d", v);
    end
  endprogram
  final $display("final v=%0d", v);
endmodule
