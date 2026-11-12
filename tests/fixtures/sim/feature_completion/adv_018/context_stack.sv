// ADV-018: the foreign code of a suspendable import runs on a native stack of
// LLG_DPI_CONTEXT_STACK_BYTES (8 MiB by default), not on the platform's
// default thread stack (musl 128 KiB, macOS 512 KiB, Windows 1 MiB).
// cs_deep fills a 6 MiB frame on that stack (12 MiB with BIG, which the test
// runs with a 16 MiB stack) and then suspends in an exported task.
`timescale 1ns/1ns
module tb;
    import "DPI-C" context task cs_deep(input int big, output int pages);
    export "DPI-C" task cs_wait;

    task automatic cs_wait(input int delay);
        #delay;
    endtask

    int pages;

    initial begin
`ifdef BIG
        cs_deep(1, pages);
`else
        cs_deep(0, pages);
`endif
        $display("%0t pages=%0d", $time, pages);
        $finish;
    end
endmodule
