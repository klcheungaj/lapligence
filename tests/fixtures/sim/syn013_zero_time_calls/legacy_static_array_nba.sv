// llg-test-fixture: tests/fixtures/sim/syn013_zero_time_calls/legacy_static_array_nba.sv
// IEEE 1364-2001 §§9.2.2, 10.2.2: a non-automatic task retains its
// local memory after return, including an NBA to a selected element.
module tb;
    task put;
        reg [7:0] data [0:1];
        begin
            data[0] <= 8'h5a;
        end
    endtask

    initial begin
        put;
        #1;
        $display("legacy=%h", put.data[0]);
        $finish(0);
    end
endmodule
