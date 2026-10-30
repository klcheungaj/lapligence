// Decision A19-D30: clocking blocks as VPI objects. An instance iterates
// its clocking blocks; vpiInternalScope lists them and the module,
// interface and program instances below it (37.12: both are scopes). An
// instance has its default
// clocking, the global clocking it declares and its own `default disable
// iff` expression. A clocking block has its clocking event as an event
// control and its default skews as delay controls; an io declaration has
// its own skews, else the block's default, for its directions, and its
// signal or expression as vpiExpr. A skew written as a delay has it as
// vpiDelay; `#1step` and an omitted skew have none. vpiInputEdge and
// vpiOutputEdge give the skew's edge, else the clocking event's edge, and
// vpiNoEdge for a direction the declaration does not have. io declarations
// iterate in declaration order and are found by name; their vpiScope and
// vpiClockingBlock is the block. A clocking block has vpiInstance and
// vpiScope but no vpiModule and no time unit; an io declaration has no
// vpiInstance, vpiParent or size; an assertion has no vpiClockingBlock.
//
// IEEE 1800-2009 37.44 (SystemVerilog-1800-2009.txt L60320-L60358): the
// clocking block diagram (vpiInputSkew, vpiOutputSkew -> delay control,
// vpiClockingEvent -> event control, instance, clocking io decl, property
// decl, sequence decl) and the clocking io decl diagram (vpiExpr -> expr,
// nets, variables, ref obj); detail 1 (L60362-L60364): "The methods,
// vpiInputSkew and vpiOutputSkew, and properties vpiInputEdge and
// vpiOutputEdge, on the clocking block apply to the default constructs.
// The same methods and properties on the clocking io decl apply to the
// clocking io decl itself."; detail 2 (L60365-L60367): "vpiExpr shall
// return the expression or ref obj referenced by the clocking io decl.
// ... Here, "enable" is represented by a clocking io decl"; Annex M
// (L76554-L76557): vpiInputEdge and vpiOutputEdge return "vpiNoEdge,
// vpiPosedge, vpiNegedge"; 14.3/14.4 (L19511-L19517): skews are relative to
// the clocking event; 37.5 and 37.9 diagrams (L57728-L57740, L57846-L57855):
// vpiGlobalClocking, vpiDefaultClocking -> clocking block,
// vpiDefaultDisableIff -> expr. The text does not give the edge of a skew
// written without one, the skews of a declaration that has none, the order
// of the io declarations, which global clocking an instance has, or what
// clocking block an assertion has; there llg follows industry-standard
// behaviour.
program prg (input logic clk);
  logic x = 1'b0;
  default clocking pcb @(negedge clk);
    input #1 x;
  endclocking
  initial #50 x = 1'b1;
endprogram

module tb;
  logic clk = 0, r = 0, s, t, rst = 0;
  logic [3:0] v = 4'h5;
  default disable iff (rst);
  clocking cb @(posedge clk);
    default input #1step output #2;
    input r;
    output negedge #3 s;
    inout t;
    input #2 rv = v[1:0];
    property cp;
      r;
    endproperty
  endclocking
  default clocking cb;
  global clocking gcb @(posedge clk); endclocking
  prg p (clk);
  assert property (r |-> r);
  initial begin
    $d30;
    $finish;
  end
endmodule
