// 100 concurrent processes wait on one named event, exceeding the retired
// 64-waiter per-event table. LRM: IEEE 1800-2009 15.5.3.
module tb;
    event ev;
    int count;
    integer i;

    initial begin
        count = 0;
        for (i = 0; i < 100; i = i + 1) begin
            fork
                begin
                    @(ev);
                    count = count + 1;
                end
            join_none
        end
        #1;
        -> ev;
        #1;
        $display("CHECK: count=%0d", count);
        $finish(0);
    end
endmodule
