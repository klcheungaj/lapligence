// llg-test-fixture: tests/fixtures/sim/syn013_zero_time_calls/static_array_nba.sv
// IEEE 1800-2009 §§6.21, 10.4.2, 13.3.2: a static task's local
// unpacked array retains its storage after the call and through NBA commit.
module tb;
    task static put;
        static logic [7:0] data [0:1] = '{8'h11, 8'h22};
        data[0] <= 8'ha5;
    endtask

    initial begin
        if (put.data[0] !== 8'h11 || put.data[1] !== 8'h22)
            $fatal(1, "static array initializer did not run before process");
        put();
        #1;
        if (put.data[0] !== 8'ha5 || put.data[1] !== 8'h22)
            $fatal(1, "static array NBA did not publish after return");
        $display("value=%h", put.data[0]);
        $finish(0);
    end
endmodule
