// Decision S3-D2: inputs of a sequential UDP whose changes one evaluation
// cannot tell apart are taken one transition each in port order, every one
// seeing the new values of the inputs before it and the old values of the
// inputs after it: two bits of one vector changed by one write, and inputs
// that are not a variable, a net or a constant bit of one. Inputs changed by
// separate writes follow the order of the writes (decision S3-D5); one
// evaluation drives only the final state, and a change that is undone
// before the UDP is evaluated is not seen.
//
// IEEE 1800-2009 4.7 (SystemVerilog-1800-2009.txt L3465-3471): "statements
// without time-control constructs in procedural blocks do not have to be
// executed as one event ... the order of interleaved execution is
// nondeterministic". 29.6 L48960: "Each table entry can have a transition
// specification on at most one input." Whether the UDP sees the clock or the
// data change first, or a zero-width clock pulse, is not determined, so the
// lines without the `llg` label hold for every conforming simulator. The
// concatenation writes the clock first (q stays 0) and the pulse is not
// seen.
primitive dff (q, clk, d);
    output q;
    reg q;
    input clk, d;
    initial q = 0;
    table
    // clk d : q : q+
        r    0 : ? : 0 ;
        r    1 : ? : 1 ;
        (?0) ? : ? : - ;
        ?    * : ? : - ;
    endtable
endprimitive

module tb;
    reg clk, d;
    wire q;
    dff u (q, clk, d);
    initial begin
        clk = 1'b0;
        d = 1'b0;
        #1 {clk, d} = 2'b11;
        #1 $display("legal=%0d", q === 1'b0 || q === 1'b1);
        $display("llg q=%b", q);
        clk = 1'b0;
        #1 clk = 1'b1;
        #1 $display("next_edge q=%b", q);
        clk = 1'b0;
        d = 1'b0;
        #1 clk = 1'b1;
        clk = 1'b0;
        #1 $display("llg pulse q=%b", q);
        $finish;
    end
endmodule
