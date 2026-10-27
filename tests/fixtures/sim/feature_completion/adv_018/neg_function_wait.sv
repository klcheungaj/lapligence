// ADV-018 negative: an imported function calls an exported task that can
// consume time (SV 35.8: never legal from an imported function): a fatal
// error at the call.
`timescale 1ns/1ns
module tb;
    import "DPI-C" context function int c_func(input int x);
    export "DPI-C" task sv_wait_e;

    int r = 0;

    task automatic sv_wait_e(input int delay);
        #delay;
    endtask

    initial begin
        $display("start");
        r = c_func(3);
        $display("after r=%0d", r);
    end
endmodule
