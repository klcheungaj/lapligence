// llg-test-fixture: tests/fixtures/sim/sv4_gmp_integration/container_owners.sv
// Packed elements of queues, dynamic and associative arrays and mailboxes are
// independent owners (IEEE 1800-2009 §7.5, §7.8, §7.10, §15.4). Reads return
// snapshots, ref formals retain element identity across structural changes,
// and mailbox payloads survive mutation of the value that was put.
`timescale 1ns/1ns
typedef logic [129:0] w_t;

module tb;
    w_t q[$];
    w_t d[];
    w_t a[int];
    mailbox #(w_t) box = new();
    w_t held, got;
    int failures = 0;

    function automatic w_t pat(input int k);
        return {k[1:0], {2{k[31:0] ^ 32'h9e37_79b9}}, {2{~k[31:0]}}};
    endfunction

    task automatic check(input string name, input w_t have, input w_t want);
        if (have !== want) begin
            failures++;
            $display("FAIL %s have=%h want=%h", name, have, want);
        end
    endtask

    task automatic bump(ref w_t slot, input int delay);
        #delay slot = slot + 130'd1;
    endtask

    initial begin
        for (int k = 0; k < 4; k++) q.push_back(pat(k));
        held = q[1];
        q[1] = 130'bz;
        check("queue_read_snapshot", held, pat(1));
        q[1] = held;
        // A pending ref write keeps the element's identity while the queue
        // grows at the front and the element moves to a new index.
        fork
            bump(q[2], 2);
        join_none
        #1;
        q.push_front(130'bx);
        q.push_front(pat(9));
        #2;
        check("queue_ref_after_push", q[4], pat(2) + 130'd1);
        check("queue_front_x", q[1], 130'bx);
        got = q.pop_front();
        check("queue_pop", got, pat(9));
        q.delete(0);
        check("queue_after_delete", q[0], pat(0));

        d = new[3];
        foreach (d[i]) d[i] = pat(i + 20);
        d = new[5](d);
        check("dyn_prefix", d[2], pat(22));
        check("dyn_default", d[4], 130'bx);
        held = d[0];
        d[0] = {66'bx, held[63:0]};
        check("dyn_snapshot", held, pat(20));
        check("dyn_partial", d[0], {66'bx, held[63:0]});

        a[-5] = pat(30);
        a[7] = {{129{1'bx}}, 1'b1};
        held = a[-5];
        a.delete(-5);
        check("assoc_snapshot", held, pat(30));
        check("assoc_missing", a.exists(-5) ? 130'd1 : 130'd0, 130'd0);
        check("assoc_partial", a[7], {{129{1'bx}}, 1'b1});

        held = pat(40);
        box.put(held);
        held = 130'bz;
        box.put(held);
        held = 130'd5;
        box.get(got);
        check("mailbox_first", got, pat(40));
        box.get(got);
        check("mailbox_second", got, 130'bz);

        if (failures == 0) $display("PASS container_owners");
        $finish(0);
    end
endmodule
