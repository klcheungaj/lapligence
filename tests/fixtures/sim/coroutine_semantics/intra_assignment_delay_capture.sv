// llg-test-fixture: tests/fixtures/sim/coroutine_semantics/intra_assignment_delay_capture.sv
// IEEE 1800-2009 §10.4.2: both forms capture the RHS when issued. A blocking
// intra-assignment delay evaluates its selected LHS after the wait, while a
// nonblocking assignment captures the selected destination when it is issued.
module tb;
    logic [7:0] blocking_value = 8'h00;
    logic [7:0] nba_value = 8'h00;
    logic source = 1'b1;
    integer index = 0;

    initial blocking_value[index] = #3 source;
    initial nba_value[index] <= #3 source;

    initial begin
        #1;
        source = 1'b0;
        index = 3;
        #3;
        if (blocking_value !== 8'h08 || nba_value !== 8'h01)
            $fatal(1, "intra-assignment capture mismatch");
        $display("PASS intra_assignment_delay_capture blocking=%h nba=%h source=%0d index=%0d",
            blocking_value, nba_value, source, index);
        $finish(0);
    end
endmodule
