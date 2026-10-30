// Decision S46-D14: toggle coverage. Every bit of every net and variable
// of a packed integral type that an instance (or one of its generate
// scopes) declares is one coverable item; arrays, reals, strings, events,
// handles and the variables of subroutines, blocks, classes and packages
// are not. A bit is covered once it has been seen at 0 and at 1 while
// coverage is collected; X and Z are neither value. When collection
// starts (time 0 for a design that names SV_COV_TOGGLE in a $coverage_*
// call, or START) every bit's present value counts as seen; afterwards a
// bit's value counts when it changes, intermediate values within a time
// step included. RESET makes every bit uncovered and leaves the present
// values unseen until they change.
//
// IEEE 1800-2009 40.2.2 (SystemVerilog-1800-2009.txt L66056-66058):
// "toggle coverage: For each bit of every signal (wire and register),
// whether that bit has both a 0 value and a 1 value. Full coverage means
// both are seen; otherwise, some implementations can query for partial
// coverage." L66060-66061: "Over instances or blocks, the coverage number
// is merely the sum of all contained primitives in that instance or
// block." 40.3.2.1 L66123-66129: "`SV_COV_START, if possible, starts
// collecting coverage information in the specified hierarchy. ...
// `SV_COV_STOP stops collecting coverage information ... `SV_COV_RESET
// resets all available coverage information in the specified hierarchy."
// Which signals count, and whether the value held when collection starts
// is seen, are not stated.
module tb;
  logic [3:0] v = 4'b0000;
  logic x1;
  bit b;
  wire [1:0] w = v[1:0];
  real r = 0.0;
  logic [1:0] mem [0:1];
  initial begin
    $display("t0 %0d/%0d", $coverage_get(`SV_COV_TOGGLE, `SV_COV_MODULE, tb),
             $coverage_get_max(`SV_COV_TOGGLE, `SV_COV_MODULE, tb));
    #1 v = 4'b0011;
    x1 = 1'b0;
    #1 x1 = 'x;
    #1 x1 = 1'b1;
    b = 1'b1;
    r = 1.0;
    mem[0] = 2'b11;
    #1 $display("a %0d/%0d", $coverage_get(`SV_COV_TOGGLE, `SV_COV_MODULE, tb),
                $coverage_get_max(`SV_COV_TOGGLE, `SV_COV_HIER, $root));
    $display("stop %0d", $coverage_control(`SV_COV_STOP, `SV_COV_TOGGLE, `SV_COV_MODULE, tb));
    v = 4'b1100;
    #1 $display("stopped %0d", $coverage_get(`SV_COV_TOGGLE, `SV_COV_MODULE, tb));
    $display("start %0d", $coverage_control(`SV_COV_START, `SV_COV_TOGGLE, `SV_COV_HIER, tb));
    $display("started %0d", $coverage_get(`SV_COV_TOGGLE, `SV_COV_MODULE, tb));
    $display("reset %0d", $coverage_control(`SV_COV_RESET, `SV_COV_TOGGLE, `SV_COV_MODULE, "tb"));
    $display("after reset %0d", $coverage_get(`SV_COV_TOGGLE, `SV_COV_MODULE, tb));
    v = 4'b0110;
    #1 $display("one change %0d", $coverage_get(`SV_COV_TOGGLE, `SV_COV_MODULE, tb));
    v = 4'b0100;
    #1 $display("back %0d check %0d", $coverage_get(`SV_COV_TOGGLE, `SV_COV_MODULE, tb),
                $coverage_control(`SV_COV_CHECK, `SV_COV_TOGGLE, `SV_COV_MODULE, tb));
    $finish;
  end
endmodule
