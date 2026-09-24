// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/static_return_continuous.sv
// IEEE 1800-2009 §§6.21 and 10.3.2: a static function result variable may be
// the target of a continuous variable assignment through hierarchy.
module tb;
    logic source;
`ifdef SYN038_DUPLICATE_DRIVER
    logic competing_source;
`endif

    function static logic f;
`ifdef SYN038_MIXED_DRIVER
        f = 1'b0;
`endif
    endfunction

    assign tb.f.f = source;
`ifdef SYN038_DUPLICATE_DRIVER
    assign tb.f.f = competing_source;
`endif

    initial begin
        source = 1'b0;
`ifdef SYN038_DUPLICATE_DRIVER
        competing_source = 1'b0;
`endif
        #1;
        if (f() !== 1'b0)
            $fatal(1, "static return slot did not receive zero");
        source = 1'b1;
        #1;
        if (f() !== 1'b1)
            $fatal(1, "static return slot did not receive one");
        $display("result=%b", f());
        $finish(0);
    end
endmodule

`ifdef SYN038_CALL_ROUTE
module call_tb;
    logic source;
    logic call_result;

    function static logic f;
    endfunction

    assign call_tb.f.f = source;
    assign call_result = f();

    initial begin
        source = 1'b0;
        #1;
        if (f() !== 1'b0 || call_result !== 1'b0)
            $fatal(1, "continuous invocation missed the zero result");
        source = 1'b1;
        #1;
        if (f() !== 1'b1 || call_result !== 1'b1)
            $fatal(1, "continuous invocation missed the one result");
        $display("result=%b call=%b", f(), call_result);
        $finish(0);
    end
endmodule
`endif
