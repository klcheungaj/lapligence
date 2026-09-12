// IEEE 1800-2009 7.10: queue slices, concatenation, and bounded overflow
// retain logical order and use `$` for the current last/end index.
module tb;
    integer q[$];
    integer r[$];
    integer bounded[$:2];

    task automatic queue_ref_edit(ref integer value);
        value = value + 1;
        q.push_back(99);
        value = 123;
    endtask

    task automatic queue_ref_shift(ref integer surviving, ref integer removed);
        q.push_front(0);
        q.delete(3);
        surviving = 222;
        removed = 999;
    endtask

    task automatic bounded_ref_shift(ref integer surviving, ref integer tail);
        bounded.push_front(0);
        surviving = 88;
        tail = 999;
    endtask

    initial begin
        q = '{1, 2, 3, 4};
        if (q[$] !== 4 || q[q.size() - 1] !== 4) begin
            $display("FAIL queue_p32 last_index");
            $finish;
        end

        r = q[1:$];
        if (r.size() !== 3 || r[0] !== 2 || r[1] !== 3 || r[$] !== 4) begin
            $display("FAIL queue_p32 suffix_slice");
            $finish;
        end
        r = q[3:1];
        if (r.size() !== 0) begin
            $display("FAIL queue_p32 reverse_slice");
            $finish;
        end
        r = q[99:0];
        if (r.size() !== 0) begin
            $display("FAIL queue_p32 out_of_range_slice");
            $finish;
        end

        r = {q[1:2], q[0:0]};
        if (r.size() !== 3 || r[0] !== 2 || r[1] !== 3 || r[2] !== 1) begin
            $display("FAIL queue_p32 concatenation");
            $finish;
        end

        bounded = '{1, 2, 3, 4};
        if (bounded.size() !== 3 || bounded[0] !== 1 || bounded[1] !== 2 || bounded[2] !== 3) begin
            $display("FAIL queue_p32 bounded_assignment");
            $finish;
        end
        bounded.push_back(9);
        bounded.push_front(0);
        bounded.insert(1, 8);
        if (bounded.size() !== 3 || bounded[0] !== 0 || bounded[1] !== 8 || bounded[2] !== 1) begin
            $display("FAIL queue_p32 bounded_mutation");
            $finish;
        end
        bounded[$] = 7;
        if (bounded.size() !== 3 || bounded[$] !== 7) begin
            $display("FAIL queue_p32 bounded_last_write");
            $finish;
        end
        bounded.delete(bounded.size() - 1);
        if (bounded.size() !== 2 || bounded[0] !== 0 || bounded[1] !== 8) begin
            $display("FAIL queue_p32 delete_last");
            $finish;
        end

        q = '{10, 20};
        queue_ref_edit(q[0]);
        if (q.size() !== 3 || q[0] !== 123 || q[1] !== 20 || q[2] !== 99) begin
            $display("FAIL queue_p32 surviving_ref");
            $finish;
        end

        q = '{1, 2, 3};
        queue_ref_shift(q[1], q[2]);
        if (q.size() !== 3 || q[0] !== 0 || q[1] !== 1 || q[2] !== 222) begin
            $display("FAIL queue_p32 shifted_and_removed_refs");
            $finish;
        end

        bounded = '{1, 2, 3};
        bounded_ref_shift(bounded[1], bounded[2]);
        if (bounded.size() !== 3 || bounded[0] !== 0 ||
            bounded[1] !== 1 || bounded[2] !== 88) begin
            $display("FAIL queue_p32 bounded_tail_ref");
            $finish;
        end

        $display("PASS queue_p32");
    end
endmodule
