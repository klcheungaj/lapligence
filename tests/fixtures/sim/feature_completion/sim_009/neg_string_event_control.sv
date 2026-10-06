// SIM-009 boundary: `@(tag)` on a subroutine string formal is legal (SV
// 9.4.2) but subroutine strings have no change marker; rejected explicitly.
module tb;
  task automatic w(input string tag);
    @(tag);
  endtask
  initial w("a");
endmodule
