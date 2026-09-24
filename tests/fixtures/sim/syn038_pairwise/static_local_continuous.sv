// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/static_local_continuous.sv
// IEEE 1800-2009 §§6.21 and 10.3.2: a static function local may be the
// hierarchical target of a continuous variable assignment.
module tb;
    logic source;
`ifdef SYN038_DUPLICATE_DRIVER
    logic competing_source;
`endif

    function logic read_state;
        logic state;
        read_state = state;
    endfunction

    assign tb.read_state.state = source;
`ifdef SYN038_DUPLICATE_DRIVER
    assign tb.read_state.state = competing_source;
`endif

    initial begin
        source = 1'b0;
`ifdef SYN038_DUPLICATE_DRIVER
        competing_source = 1'b0;
`endif
        #1;
        if (read_state() !== 1'b0)
            $fatal(1, "static function local did not receive zero");
        source = 1'b1;
        #1;
        if (read_state() !== 1'b1)
            $fatal(1, "static function local did not receive one");
        $display("state=%b", read_state());
        $finish(0);
    end
endmodule
