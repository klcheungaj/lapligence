// Decision A21-D8 (scope): only subroutines that a process reaches through
// calls whose callee is known statically are checked for writes to a
// variable connected to an output port, as module tasks nothing calls are
// not checked. Class task
// `D::run` assigns `s`, the actual of `drv`'s output port, but nothing calls
// it, so the design runs: `drv` writes 0 at the posedge at 5ns, and at 13ns
// `s` is 0.
//
// IEEE 1800-2009 23.3.3.2 (SystemVerilog-1800-2009.txt L41485-41486):
// "Procedural or continuous assignments to a variable connected to the
// output port of an instance shall be illegal." The text forbids the
// assignment itself; llg does not check a subroutine nothing calls.
`timescale 1ns/1ps
module drv(input logic clk, output logic o);
  always @(posedge clk) o <= 1'b0;
endmodule
module tb;
  logic clk = 1'b0;
  logic s;
  drv d(clk, s);
  class D;
    task run();
      #12 s = 1'b1;
    endtask
  endclass
  always #5 clk = ~clk;
  initial begin
    static D h = new;
    #13 $display("s=%b", s);
    $finish;
  end
endmodule
