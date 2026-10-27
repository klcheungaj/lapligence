// ADV-018 A01: two foreign call chains suspended at once resume in crossing
// order: chain 1 enters first and resumes first while chain 2, entered
// later, is still suspended. Chains 4 and 3 resume in the opposite order
// (last in, first out). Each C frame checks its own locals after the
// export returns.
`timescale 1ns/1ns
module tb;
    import "DPI-C" context task c_chain(input int id, input int delay);
    export "DPI-C" task sv_wait;

    task automatic sv_wait(input int id, input int delay);
        $display("%0t sv_wait %0d waits %0d", $time, id, delay);
        #delay;
        $display("%0t sv_wait %0d resumes", $time, id);
    endtask

    initial begin
        c_chain(1, 10);
        $display("%0t chain 1 returned", $time);
    end

    initial begin
        #5;
        c_chain(2, 20);
        $display("%0t chain 2 returned", $time);
    end

    initial begin
        #40;
        c_chain(4, 30);
        $display("%0t chain 4 returned", $time);
    end

    initial begin
        #50;
        c_chain(3, 5);
        $display("%0t chain 3 returned", $time);
    end

    initial #100 $finish;
endmodule
