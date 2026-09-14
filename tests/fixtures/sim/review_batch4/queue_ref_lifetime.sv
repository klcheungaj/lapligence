// llg-test-fixture: tests/fixtures/sim/review_batch4/queue_ref_lifetime.sv
module tb;
    integer q[$];
    integer answer;
    process worker;
    task automatic suspended(ref integer item);
        #3;
        item = 37;
        if (item !== 37) $fatal(1, "suspended outdated value");
        answer = item;
    endtask
    task automatic cancelled(ref integer item);
        #20;
        item = 88;
        $fatal(1, "cancelled reference resumed");
    endtask
    initial begin
        q = '{5};
        fork suspended(q[0]); join_none
        #1;
        q.delete();
        #3;
        if (answer !== 37 || q.size() != 0) $fatal(1, "suspended reference result");
        q.push_back(9);
        fork
            begin worker = process::self(); cancelled(q[0]); end
        join_none
        #1;
        q.delete();
        worker.kill();
        q.push_back(17);
        #21;
        if (q.size() != 1 || q[0] !== 17) $fatal(1, "cancelled reference corrupted queue");
        $display("queue reference lifetime ok");
        $finish(0);
    end
endmodule
