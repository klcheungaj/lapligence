// llg-test-fixture: ADV-020 user system functions in evaluated contexts.
// systf_evaluators.c registers `$plus1(v)`, whose calltf returns v + 1.
// The model calls it from an event control, a force source, a wait
// condition (through a function) and a `$monitor` argument; each context
// re-evaluates the call when its operand changes (IEEE 1800-2009 9.4.2,
// 10.6.2, 36.2). Expected output: systf_evaluators.out.
`timescale 1ns/1ns
module tb;
    int a = 0;
    int y;
    int hits = 0;
    int base;

    function automatic int twice(input int v);
        return $plus1(v) * 2;
    endfunction

    always @($plus1(a)) hits++;

    initial begin
        #1 force y = $plus1(a);
        #1 $display("t=%0t y=%0d", $time, y);
        base = hits;
        a = 5;
        #1 $display("t=%0t y=%0d event=%0d", $time, y, hits - base);
        $monitor("monitor t=%0t plus1=%0d", $time, $plus1(a));
        fork
            begin
                wait (twice(a) == 14);
                $display("wait done t=%0t a=%0d", $time, a);
            end
            begin
                #2 a = 6;
            end
        join
        #1 $display("t=%0t y=%0d event=%0d", $time, y, hits - base);
        release y;
        $finish;
    end
endmodule
