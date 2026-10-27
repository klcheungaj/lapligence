// ADV-018 negative: the C code of a killed process's chain calls an export
// again after its waiting exported task returned 1 (SV 35.9 item d): a
// fatal error, and that C code never runs on.
`timescale 1ns/1ns
module tb;
    import "DPI-C" context task c_again();
    export "DPI-C" task sv_wait_e;

    process p;

    task automatic sv_wait_e(input int delay);
        #delay;
    endtask

    initial begin
        $display("start");
        p = process::self();
        c_again();
        $display("not reached");
    end
    initial begin
        #10;
        p.kill();
        $display("after kill");
    end
endmodule
