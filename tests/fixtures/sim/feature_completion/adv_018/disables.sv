// ADV-018 A02: the disable protocol of suspended chains (SV 35.9). (1) A
// disable of the block around the import, from another process, ends the
// waiting exported task: it returns 1, the C code sees svIsDisabledState()
// and returns 1, and the output is not copied back. (2) A disable of the
// exported task itself returns 0 and the import completes. (3) A process
// killed while its chain waits unwinds it at the kill: the exported task
// returns 1 and the C code returns 1. (4) A chain still waiting when the
// simulation finishes never resumes its C code.
`timescale 1ns/1ns
module tb;
    import "DPI-C" context task c_hold_task(input int id, output int o);
    export "DPI-C" task sv_hold;

    int o1 = 5;
    int o2 = 6;
    int o3 = 7;
    int o4 = 8;
    process p3;

    task automatic sv_hold(input int id);
        #100;
        $display("%0t sv_hold %0d done", $time, id);
    endtask

    initial begin
        begin : blk1
            c_hold_task(1, o1);
            $display("blk1 not reached");
        end
        $display("%0t after blk1 o1=%0d", $time, o1);
    end
    initial #10 disable blk1;

    initial begin
        #20;
        c_hold_task(2, o2);
        $display("%0t after task 2 o2=%0d", $time, o2);
    end
    initial #30 disable sv_hold;

    initial begin
        p3 = process::self();
        #40;
        c_hold_task(3, o3);
        $display("p3 not reached");
    end
    initial begin
        #50;
        p3.kill();
        $display("%0t killed p3 o3=%0d status=%s", $time, o3, p3.status().name());
    end

    initial begin
        #60;
        c_hold_task(4, o4);
        $display("task 4 not reached");
    end
    initial #70 $finish;
    final $display("final o4=%0d", o4);
endmodule
