// 100 concurrent forked processes exceed the old fixed process registry and
// the runtime's initial slot-table capacity. LRM: IEEE 1800-2009 9.3.
module tb;
    int count;
    integer i;

    initial begin
        count = 0;
        for (i = 0; i < 100; i = i + 1) begin
            fork
                begin
                    #1;
                    count = count + 1;
                end
            join_none
        end
        #2;
        $display("CHECK: count=%0d", count);
        $finish(0);
    end
endmodule
