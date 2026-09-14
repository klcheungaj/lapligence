// llg-test-fixture: tests/fixtures/sim/review_batch4/queue_outdated.sv
module tb;
    integer q[$];
    integer result;
    function automatic integer detach_aliases(ref integer a, ref integer b);
        q.delete(0);
        a = 12;
        b++;
        if (a !== 13 || b !== 13) $fatal(1, "detached aliases lost shared value");
        q.push_back(99);
        a = 14;
        if (b !== 14 || q[0] !== 99) $fatal(1, "outdated write reached new element");
        return a;
    endfunction
    function automatic integer replace_queue(ref integer a);
        q = q;
        a = 42;
        if (q[0] !== 7) $fatal(1, "self assignment did not detach reference");
        return a;
    endfunction
    function automatic integer pop_queue(ref integer a);
        integer old_value;
        old_value = q.pop_back();
        if (a !== old_value) $fatal(1, "pop lost retained value");
        a = 55;
        return a;
    endfunction
    initial begin
        q = '{7};
        result = detach_aliases(q[0], q[0]);
        if (result !== 14 || q.size() != 1 || q[0] !== 99) $fatal(1, "alias result");
        q = '{7};
        result = replace_queue(q[0]);
        if (result !== 42) $fatal(1, "replacement result");
        result = pop_queue(q[0]);
        if (result !== 55 || q.size() != 0) $fatal(1, "pop result");
        $display("queue outdated references ok");
        $finish(0);
    end
endmodule
