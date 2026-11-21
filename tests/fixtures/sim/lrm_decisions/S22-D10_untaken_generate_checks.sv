// Decision S22-D10: nothing of a generate block that elaboration does not
// instantiate exists, so the whole-design checks and collections ignore it.
// With every parameter 0, each block below is untaken:
//   - its gates drive nothing: an always_comb procedure is the only writer
//     of y and z (an untaken if and an unselected case item);
//   - its calls write nothing: built-in methods that modify a const queue
//     and a const string are no illegal writes;
//   - its global clocking declaration is not a second one in the model;
//   - its assignment through a virtual interface to an input port variable
//     is no illegal write;
//   - its covergroup `new`, whose ref argument names the block's own
//     variable, constructs nothing.
// Setting GATE, CONST, GCLK or VIF to 1 instantiates the block and makes the
// design illegal (the nearest illegal cases); CG=1 adds a second instance.
//
// IEEE 1800-2009 27.3 (SystemVerilog-1800-2009.txt L45986-45988): "The
// elaboration of a generate construct results in zero or more instances of
// a generate block. An instance of a generate block is similar in some ways
// to an instance of a module. It creates a new level of hierarchy. It brings
// the objects, behavioral constructs, and module instances within the block
// into existence." 27.5 L46364-46366: "The conditional generate constructs,
// if-generate and case-generate, select at most one generate block from a
// set of alternative generate blocks based on constant expressions evaluated
// during elaboration. The selected generate block, if any, is instantiated
// into the model." 9.2.2.2 L11212-11213: "Variables on the left-hand side of
// assignments within an always_comb procedure, including variables from the
// contents of a called function, shall not be written to by any other
// processes". 14.14 L19969-19970: "There shall be at most one global
// clocking declaration anywhere in an entire elaborated SystemVerilog model.
// It shall be an error if there is more than one such global clocking
// declaration." 23.3.3.2 L41469-41481: "Assignments to variables declared as
// input ports shall be illegal."
`timescale 1ns/1ns
interface bus_if (input logic i);
  logic o;
endinterface
module tb #(parameter bit GATE = 0, CONST = 0, GCLK = 0, VIF = 0, CG = 0);
  logic clk = 1'b0;
  always #5 clk = ~clk;

  logic y, z;
  if (GATE) begin : g
    buf (y, 1'b0);
  end
  case (GATE)
    1'b1: begin : c
      not (z, 1'b1);
    end
    default: begin : d
    end
  endcase
  always_comb y = 1'b1;
  always_comb z = 1'b1;

  const int q[$] = '{1, 2};
  const string s = "ab";
  if (CONST) begin : w
    initial begin
      q.push_back(3);
      s.putc(0, "x");
    end
  end

  if (GCLK) begin : gc
    global clocking gs @(negedge clk); endclocking
  end
  global clocking gm @(posedge clk); endclocking
  int ticks = 0;
  always @($global_clock) ticks++;

  logic a = 1'b1;
  bus_if b (a);
  virtual bus_if v = b;
  if (VIF) begin : vw
    initial v.i = 1'b0;
  end

  covergroup cov (ref int r) @(posedge clk);
    coverpoint r { bins b[] = {[0:3]}; }
  endgroup
  int n = 0;
  cov c0 = new(n);
  if (CG) begin : cg
    int m = 0;
    cov c1 = new(m);
  end

  initial begin
    v.o = 1'b0;
    repeat (4) begin
      @(negedge clk);
      n++;
    end
    $display("y=%b z=%b", y, z);
    $display("q.size=%0d q[1]=%0d s=%s", q.size(), q[1], s);
    $display("ticks=%0d", ticks);
    $display("o=%b i=%b", v.o, v.i);
    $display("cov=%0.2f", c0.get_coverage());
    $finish;
  end
endmodule
