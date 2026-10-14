// SIM-041 negative: an exported task that can consume time. Suspending a
// task called from C needs a suspended native stack (ADV-018).
module tb;
    export "DPI-C" task wait_one;

    task wait_one();
        #1;
    endtask

    initial $finish;
endmodule
