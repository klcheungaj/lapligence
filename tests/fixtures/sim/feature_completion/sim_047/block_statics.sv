// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_047/block_statics.sv
// SIM-047 A03: a dump covers the static variables of named and unnamed
// blocks, tasks and functions and named events (SV 21.7.2.2 scope and
// variable types), and $dumpfile takes a computed name (SV 21.7.1.1).
`timescale 1ns/1ns
`ifndef FILE
`define FILE "statics.vcd"
`endif
module cell_unit;
    event done;
    initial begin : run
        logic [1:0] state;
        state = 2'd1;
        #3 state = 2'd2;
        ->done;
    end
endmodule

module tb;
    logic [3:0] count;
    event tick;
    cell_unit u();

    function int accumulate(int step);
        static int total;
        total = total + step;
        return total;
    endfunction

    function automatic int plain(int step);
        int doubled;
        doubled = step * 2;
        return doubled;
    endfunction

    task pulse(input int level);
        static byte seen;
        seen = level;
        begin : inner
            static shortint next;
            next = level + 1;
            #1;
        end
    endtask

    always @(tick) count = count + 1;

    initial begin : main
        static int keep = 5;
        reg [7:0] mask;
        real ratio;
        event mark;
        string file;
        count = 0;
        file = `FILE;
        $dumpfile(file);
        $dumpvars(0, tb);
        #1 mask = 8'h3c;
        ratio = 1.5;
        ->tick;
        #1 keep = accumulate(3) + plain(1);
        keep = accumulate(0) + plain(1);
        ->mark;
        #1 pulse(4);
        begin : nested
            static int deep;
            deep = keep;
            #1 deep = deep + 1;
            begin
                static int anonymous;
                anonymous = 7;
                #1;
            end
        end
        fork : parallel
            begin : first
                static int value;
                #1 value = 2;
            end
            #2;
        join
        ->tick;
        #1 ->tick;
        ->tick;
        #1 $display("count=%0d keep=%0d", count, keep);
        $finish;
    end
endmodule
