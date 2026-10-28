// Decision A17-D4: assertion relations. vpiClockingEvent of a concurrent
// assertion is the clock that evaluates it: the one written at the top of
// its property (also inside the named property it instantiates), else the
// edge of the enclosing always procedure, else the default clocking;
// vpiIsClockInferred is 1 for the last two. vpiDisableCondition is the
// disable iff written in the statement only: one inside a named property
// and the default disable iff are not reported. vpiProperty is a property
// instance when the statement's property is exactly one named property,
// else a property specification holding only the clocking event and
// disable condition written in the statement and the property expression.
// failExpr of cbAssertionFailure is that property expression (or the
// property instance), and the condition of an immediate assertion.
// vpiEndLine and vpiEndColumn give the statement's last character, its
// semicolon included. An empty action `;` and a missing else action are
// NULL.
//
// IEEE 1800-2009 37.46 (SystemVerilog-1800-2009.txt L60456-L60457):
// "Clocking event is always the actual clocking event on which the
// assertion is being evaluated, regardless of whether this is explicit or
// implicit (inferred)." 39.4.2 L65739: "On a cbAssertionFailure callback,
// the attemptStartTime and detail.failExpr fields are valid." 37.45 lists
// vpiStartLine, vpiColumn, vpiEndLine and vpiEndColumn without defining
// them. The text is silent on which disable iff the relation names and on
// empty actions (industry-standard behaviour followed); which expression
// failExpr names, whether vpiProperty is an instance or a specification,
// and the end column are llg choices.
//
// Build A17-D4_assertion_relations.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d4()` declared as a system task).
`timescale 1ns/1ns
module tb;
    logic clk = 0, a = 1, b = 1, rst = 0;
    always #5 clk = ~clk;
    default clocking cb @(negedge clk); endclocking
    default disable iff (rst);
    property p(x);
        @(posedge clk) x;
    endproperty
    q1: assert property (p(b));
    q2: assert property (@(posedge clk) a |-> b) else $display("%0t q2 else", $time);
    q3: assert property (a ##1 b);
    always @(posedge clk) q4: assert property (b);
    always @(posedge clk) q5: assert (b);
    initial begin
        $d4;
        #12 b = 1'b0;
        #10 $finish;
    end
endmodule
