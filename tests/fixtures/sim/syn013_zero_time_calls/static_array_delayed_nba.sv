// llg-test-fixture: tests/fixtures/sim/syn013_zero_time_calls/static_array_delayed_nba.sv
// IEEE 1800-2009 §§10.4.2 and 13.3.2: delayed NBAs to persistent
// selected array cells publish at their scheduled time and in issue order.
module tb;
    task static schedule;
        static logic [7:0] data [0:1] = '{8'h10, 8'h20};
        #1;
        data[0] <= 8'h11;
        data[0] <= 8'h22;
        data[1] <= #2 8'h33;
        data[1] <= #3 8'h44;
    endtask

    initial begin
        if (schedule.data[0] !== 8'h10 || schedule.data[1] !== 8'h20)
            $fatal(1, "timed task static array initializer missing");
        schedule();
        if (schedule.data[0] !== 8'h10 || schedule.data[1] !== 8'h20)
            $fatal(1, "NBA wrote during Active region");
        #1;
        if (schedule.data[0] !== 8'h22 || schedule.data[1] !== 8'h20)
            $fatal(1, "same-slot NBA order mismatch");
        #1;
        if (schedule.data[1] !== 8'h20)
            $fatal(1, "delayed NBA published before its region");
        #1;
        if (schedule.data[1] !== 8'h33)
            $fatal(1, "first delayed NBA missing");
        #1;
        if (schedule.data[1] !== 8'h44)
            $fatal(1, "second delayed NBA missing");
        $display("ordered=%h delayed=%h", schedule.data[0], schedule.data[1]);
        $finish(0);
    end
endmodule
