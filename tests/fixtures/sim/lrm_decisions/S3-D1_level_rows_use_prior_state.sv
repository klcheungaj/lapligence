// Decision S3-D1: when one input of a sequential UDP changes, the next state
// is chosen by one lookup against the state held before the change: a level
// row matching the new inputs and that state decides; otherwise an edge row
// for the transition decides; otherwise the result is x. The level rows are
// not looked up again with the state an edge row produced.
//
// IEEE 1800-2009 29.9 (SystemVerilog-1800-2009.txt L49196-49198): "When the
// input changes, the edge-sensitive cases are processed first, followed by
// level-sensitive cases. Thus, when level-sensitive and edge-sensitive cases
// specify different output values, the result is specified by the
// level-sensitive case." The text does not say whether the level cases see
// the state the edge case produced. Here the rising clock with d = 0 in state
// 0 matches only the edge row (next state 1); the level row `1 0 : 1 : 0`
// matches only state 1. llg prints 1; re-reading the level rows with the new
// state would print 0.
primitive edge_then_level (q, c, d);
    output q;
    reg q;
    input c, d;
    initial q = 0;
    table
    // c d : q : q+
        r 0 : 0 : 1 ;
        1 0 : 1 : 0 ;
        0 ? : ? : - ;
        f ? : ? : - ;
        ? * : ? : - ;
    endtable
endprimitive

module tb;
    reg c, d;
    wire q;
    edge_then_level u (q, c, d);
    initial begin
        d = 1'b0;
        #1 c = 1'b0;
        #1 $display("before q=%b", q);
        c = 1'b1;
        #1 $display("after_rise q=%b", q);
        $finish;
    end
endmodule
