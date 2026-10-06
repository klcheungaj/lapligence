// SIM-010: a task's queue used by fork branches is one container shared by
// the task and every branch (SV 6.21, 9.3.2), with joined and detached
// branches, and it lives until the last detached branch is done.
module tb;
    task automatic producer_consumer();
        int q[$];
        int got[$];
        fork
            begin
                for (int i = 1; i <= 3; i++) begin
                    #1 q.push_back(i * 10);
                end
            end
            begin
                repeat (3) begin
                    #1;
                    #0 got.push_back(q.pop_front());
                end
            end
        join
        $display("got %0d %0d %0d", got[0], got[1], got[2]);
    endtask
    task automatic detached();
        int log[$];
        fork
            begin
                #2 log.push_back(7);
                $display("log %0d at %0d", log.size(), $time);
            end
        join_none
        log.push_back(1);
    endtask
    initial begin
        producer_consumer();
        detached();
        #5;
    end
endmodule
