// ADV-018 A02: nested C -> SV -> C -> SV chains of two instances, with a
// fork of time waits in the outer export and an event wait in the inner
// one. Each inner C call moves its chain to the other instance with
// svSetScope, so the chain that started second waits on the event that is
// triggered first and returns first. Export inputs (a string included) and
// outputs cross the suspensions; every C frame checks its own locals and
// its scope after each export returns. A zero-time exported function runs
// inside the inner C call before it waits.
`timescale 1ns/1ns
module leaf #(parameter int ID = 0) ();
    import "DPI-C" context task c_outer(input int depth, output int result);
    import "DPI-C" context task c_inner(input int depth, output int result);
    export "DPI-C" task sv_step;
    export "DPI-C" task sv_inner_wait;
    export "DPI-C" function sv_count;

    event go;
    int result = 0;
    int count = 0;

    function int sv_count(input int depth);
        count += depth;
        return count * 100 + ID;
    endfunction

    task automatic sv_step(input int depth, input string tag, output int got);
        int r;
        $display("%0t %m depth=%0d tag=%s", $time, depth, tag);
        fork
            #2;
            #3;
        join
        c_inner(depth + 1, r);
        $display("%0t %m back tag=%s r=%0d", $time, tag, r);
        got = r + 1;
    endtask

    task automatic sv_inner_wait(input int depth, output int got);
        $display("%0t %m depth=%0d waits for go", $time, depth);
        @(go);
        got = depth * 10 + ID;
        $display("%0t %m woke got=%0d", $time, got);
    endtask

    initial begin
        #(ID);
        c_outer(0, result);
        $display("%0t %m result=%0d", $time, result);
    end
endmodule

module tb;
    leaf #(1) u1();
    leaf #(2) u2();

    initial begin
        #20 -> u1.go;
        #10 -> u2.go;
        #10 $finish;
    end
endmodule
