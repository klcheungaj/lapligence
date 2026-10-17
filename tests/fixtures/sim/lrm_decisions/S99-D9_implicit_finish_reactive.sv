// Decision S99-D9: the implicit `$finish` that ends a simulation with
// programs (24.3) takes effect as soon as the last program thread ends, in
// the Reactive region that runs it. Reactive-region events still pending in
// that time step, such as the fail action of a concurrent assertion
// evaluated in the same step, do not run; final procedures do. Module code
// of later time steps never runs. S32-D1 covers the Re-NBA updates and
// design processes of the same time slot.
//
// IEEE 1800-2009 24.3 (SystemVerilog-1800-2009.txt L43228-43232): "When all
// initial procedures within a program have reached their end, that program
// shall immediately terminate all descendent threads of initial procedures
// within that program. If there is at least one initial procedure within at
// least one program block, the entire simulation shall terminate by means of
// an implicit call to the $finish system task immediately after all the
// threads and all their descendent threads originating from all initial
// procedures within all programs have ended." 4.4.2.6 L3184-3188: "The
// Reactive region holds the current reactive region set events being
// evaluated and can be processed in any order. The code specified by
// blocking assignments in program blocks and the code in action blocks of
// concurrent assertions are scheduled in the Reactive region." Both the
// program thread and the fail action are Reactive events of time 5, which
// may run in either order; llg runs the program thread first, so the action
// line, labelled `llg`, is absent from llg's output. A simulator that runs
// the action first prints that line before `program done`.
`timescale 1ns/1ns

module tb;
  logic clk = 0;
  always #5 clk = ~clk;
  a1: assert property (@(posedge clk) 0)
    else $display("llg [%0t] a1 fail action", $time);
  always @(posedge clk) $display("[%0t] module always", $time);
  initial #7 $display("[%0t] after the program", $time);
  program automatic p;
    initial begin
      @(posedge clk);
      $display("[%0t] program done", $time);
    end
  endprogram
  p u();
  final $display("[%0t] final", $time);
endmodule
