// llg-test-fixture: tests/fixtures/sim/syn013_zero_time_calls/static_array_matrix.sv
// IEEE 1800-2009 §§6.21, 10.4.2, 13.3.2: static array elements and
// rows retain declaration and instance identity across calls and NBA regions.
module worker;
    integer wakeups = 0;

    task static put(input logic [7:0] value);
        logic [7:0] data [0:1][0:1];
        data[0][0] = value;
        data[0][1] = value + 8'd1;
        data[1] <= data[0];
        data[0][0] <= value + 8'd2;
        data[0][1][3:0] <= 4'hf;
    endtask

    always @(put.data[0][0]) wakeups = wakeups + 1;
endmodule

module tb;
    worker first();
    worker second();

    initial begin
        #1;
        first.put(8'h11);
        second.put(8'h41);
        #1;
        if (first.put.data[0][0] !== 8'h13 ||
            first.put.data[0][1] !== 8'h1f ||
            first.put.data[1][0] !== 8'h11 ||
            first.put.data[1][1] !== 8'h12 ||
            second.put.data[0][0] !== 8'h43 ||
            second.put.data[1][0] !== 8'h41 ||
            first.wakeups !== 2 || second.wakeups !== 2)
            $fatal(1, "first static array NBA or wakeup mismatch");

        first.put(8'h21);
        #0;
        if (first.put.data[0][0] !== 8'h21 ||
            first.put.data[1][0] !== 8'h11)
            $fatal(1, "NBA published before its region");
        #1;
        if (first.put.data[0][0] !== 8'h23 ||
            first.put.data[1][0] !== 8'h21 ||
            second.put.data[0][0] !== 8'h43 ||
            first.wakeups !== 4 || second.wakeups !== 2)
            $fatal(1, "repeated call or per-instance storage mismatch");
        $display("first=%h/%h second=%h wakeups=%0d/%0d",
                 first.put.data[0][0], first.put.data[1][0],
                 second.put.data[0][0], first.wakeups, second.wakeups);
        $finish(0);
    end
endmodule
