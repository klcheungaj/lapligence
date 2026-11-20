// Decision A21-D3 (exception): an output port whose actual variable is also
// written through a virtual interface keeps its continuous assignment. The
// write through the handle holds until the formal changes value again; the
// formal rewritten with its own value does not overwrite it. A later change
// of the formal reaches the actual.
//
// IEEE 1800-2009 23.3.3.2 (SystemVerilog-1800-2009.txt L41483-41486):
//   "An output port can be connected to a variable (or a concatenation) of a
//   compatible data type. A continuous assignment shall be implied when a
//   variable is connected to the output port of an instance. Procedural or
//   continuous assignments to a variable connected to the output port of an
//   instance shall be illegal."
// IEEE 1800-2009 10.3.2 (L12819-12821):
//   "Assignments on nets or variables shall be continuous and automatic. In
//   other words, whenever an operand in the right-hand expression changes
//   value, the whole right-hand side shall be evaluated. If the new value is
//   different from the previous value, then the new value shall be assigned
//   to the left-hand side."
// IEEE 1800-2009 25.9 (L44930-44931):
//   "Once a virtual interface has been initialized, all the components of the
//   underlying interface instance are directly available to the virtual
//   interface via the dot notation."
//
// The writes through the handle are illegal by 23.3.3.2, but no compiler can
// see them in the instance tree. llg follows the industry-standard behaviour,
// which runs such designs: the port stays the continuous assignment of
// 10.3.2, so a write through the handle lasts until the formal next changes.
`timescale 1ns/1ns
module drv(input wire clk, output reg o);
  int n = 0;
  always @(posedge clk) begin
    n++;
    if (n == 6) o <= 1'b0;
    else o <= 1'bz;
  end
endmodule

interface ifc;
  logic clk = 0;
  logic s;
endinterface

class D;
  virtual ifc v;
  task run();
    @(posedge v.clk);
    @(posedge v.clk); v.s <= 1;
    @(posedge v.clk); v.s <= 0;
    @(posedge v.clk); v.s <= 1'bz;
    @(posedge v.clk); v.s = 1;
    @(posedge v.clk); v.s <= 1;
    @(posedge v.clk);
  endtask
endclass

module tb;
  ifc i();
  drv d(i.clk, i.s);
  always #5 i.clk = ~i.clk;
  always @(negedge i.clk) if ($time > 0) $display("%0t s=%b o=%b", $time, i.s, d.o);
  initial begin
    static D h = new;
    h.v = i;
    fork h.run(); join_none
    #78 $finish;
  end
endmodule
