// llg-test-fixture: tests/fixtures/sim/syn013_zero_time_calls/automatic_formal_nba_rejected.sv
// IEEE 1800-2009 §10.4.2 also forbids an NBA to an automatic input formal.
module tb;
    task automatic put(input logic [7:0] value);
        value <= 8'h35;
    endtask
    initial put(8'h12);
endmodule
