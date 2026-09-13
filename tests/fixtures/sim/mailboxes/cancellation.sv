// llg-test-fixture: tests/fixtures/sim/mailboxes/cancellation.sv
// IEEE 1800-2009 §15.4 and Annex G.4: cancellation removes blocked mailbox
// waiters and pending message ownership without changing queued FIFO state.
module tb;
    mailbox #(int) bounded = new(1);
    int value;

    initial begin
        fork
            begin
                bounded.get(value);
                $display("bad_get=%0d", value);
            end
        join_none
        #1 disable fork;
        $display("get_killed n=%0d", bounded.num());
        bounded.put(5);
        $display("after_get_kill n=%0d", bounded.num());

        fork
            begin
                bounded.put(6);
                $display("bad_put");
            end
        join_none
        #1 disable fork;
        $display("put_killed n=%0d try=%0d", bounded.num(), bounded.try_put(7));
        bounded.get(value);
        $display("retained=%0d n=%0d", value, bounded.num());
        $finish;
    end
endmodule
