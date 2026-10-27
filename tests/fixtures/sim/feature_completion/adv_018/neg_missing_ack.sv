// ADV-018 negative: an imported task whose exported task was ended by a
// disable while it waited returns 0 instead of 1 (SV 35.9 item b): a fatal
// error.
`timescale 1ns/1ns
module tb;
    import "DPI-C" context task c_no_ack();
    export "DPI-C" task sv_wait_e;

    task automatic sv_wait_e(input int delay);
        #delay;
    endtask

    initial begin
        $display("start");
        begin : blk
            c_no_ack();
        end
        $display("after blk");
    end
    initial #10 disable blk;
endmodule
