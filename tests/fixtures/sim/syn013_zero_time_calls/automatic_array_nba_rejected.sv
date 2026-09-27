// llg-test-fixture: tests/fixtures/sim/syn013_zero_time_calls/automatic_array_nba_rejected.sv
// IEEE 1800-2009 §10.4.2 forbids an NBA to automatic variable storage.
module tb;
    task automatic put;
        logic [7:0] data [0:1];
        data[0] <= 8'h35;
    endtask
    initial put();
endmodule
