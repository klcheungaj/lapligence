// Decision A21-D8: a write in a subroutine that a process calls (here a
// class task) to a variable connected to an output port is a compile error,
// reported at the write. Here `s` is the actual of `drv`'s output port and
// class task `D::run`, forked by the initial procedure, assigns it; the
// design must not run and prints nothing.
//
// IEEE 1800-2009 23.3.3.2 (SystemVerilog-1800-2009.txt L41483-41486): "A
// continuous assignment shall be implied when a variable is connected to
// the output port of an instance. Procedural or continuous assignments to a
// variable connected to the output port of an instance shall be illegal."
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
    fork h.run(); join_none
    #13 $display("s=%b", s);
    $finish;
  end
endmodule
