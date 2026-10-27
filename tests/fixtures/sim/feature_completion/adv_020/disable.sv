// llg-test-fixture: ADV-020 cbDisable. disable.c ($watch_disable, $inner)
// places cbDisable records on named blocks, a named fork, a task and a
// system task call, and reports each `disable` that ends one (IEEE
// 1800-2009 38.36.1). Expected output: disable.out.
`timescale 1ns/1ns
module tb;
    logic [3:0] v;

    task work(input int n);
        #5 $display("work %0d finished at %0t", n, $time);
    endtask

    initial begin
        v = 0;
        $watch_disable;
        fork
            begin : blk
                #10 $display("blk finished at %0t", $time);
            end
            #1 disable blk;
            #2 disable blk;
            work(1);
            #3 disable work;
            fork : fk
                #10 $display("fk finished at %0t", $time);
            join
            #4 disable fk;
            begin : outer2
                begin : inner
                    $inner;
                    #10 $display("inner finished at %0t", $time);
                end
            end
            #5 disable outer2;
            #6 begin : selfb
                $display("selfb starts at %0t", $time);
                disable selfb;
                $display("selfb continued");
            end
        join
        $display("done at %0t", $time);
        $finish;
    end
endmodule
