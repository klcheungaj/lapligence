// llg-test-fixture: tests/fixtures/sim/syn013_zero_time_calls/explicit_static_array_nba.sv
// IEEE 1800-2009 §§6.21 and 10.4.2: an explicitly static declaration
// remains persistent inside an otherwise automatic task.
module tb;
    task automatic put(input logic [7:0] value);
        static logic [7:0] data [0:1];
        data[0] <= value;
    endtask

    initial begin
        put(8'h35);
        #1;
        if (put.data[0] !== 8'h35)
            $fatal(1, "first explicit-static NBA missing");
        put(8'h67);
        #1;
        if (put.data[0] !== 8'h67)
            $fatal(1, "second explicit-static NBA missing");
        $display("value=%h", put.data[0]);
        $finish(0);
    end
endmodule
