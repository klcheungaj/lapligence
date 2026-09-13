// llg-test-fixture: tests/fixtures/sim/static_review/recursive_task_disable.sv
// Static-review regression source; not executed in the review session.
module tb;
    integer continued;
    task automatic descend(input integer depth);
        if (depth != 0)
            descend(depth - 1);
        else
            disable descend;
        continued = continued + 1;
    endtask
    initial begin
        continued = 0;
        descend(3);
        $display("continued=%0d", continued);
        $finish(0);
    end
endmodule
