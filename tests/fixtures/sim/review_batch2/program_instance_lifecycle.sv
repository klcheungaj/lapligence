// R04: equal definitions still have distinct lifecycle identities.
program worker(input bit exit_early);
    initial begin
        fork
            begin #3; tb.detached++; end
        join_none
        if (exit_early) begin
            #1;
            tb.exit_from_design();
            tb.unreachable++;
        end else begin
            #5;
            tb.finished++;
        end
    end
endprogram
module tb;
    int detached = 0, finished = 0, unreachable = 0, outside = 0;
    worker a(1'b1);
    worker b(1'b0);
    task exit_from_design();
        $exit;
    endtask
    initial begin
        exit_from_design();
        outside = 1;
    end
    final $display("detached=%0d finished=%0d unreachable=%0d outside=%0d",
                   detached, finished, unreachable, outside);
endmodule
