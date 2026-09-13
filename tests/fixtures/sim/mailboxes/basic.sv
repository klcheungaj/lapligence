// llg-test-fixture: tests/fixtures/sim/mailboxes/basic.sv
// IEEE 1800-2009 §15.4 and Annex G.4: typed/untyped mailbox construction,
// FIFO bounds, nonblocking queries, value copies, and handle identity.
class item;
    int id;
endclass

module tb;
    mailbox #(int) bounded = new(2);
    mailbox inbox = new(0);
    int first;
    int second;
    int object_id;
    string source;
    string copied;
    item original;
    item received;

    initial begin
        $display("start=%0d", bounded.num());
        $display("put1=%0d n=%0d", bounded.try_put(10), bounded.num());
        $display("put2=%0d n=%0d", bounded.try_put(20), bounded.num());
        $display("full=%0d n=%0d", bounded.try_put(30), bounded.num());
        bounded.peek(first);
        $display("peek=%0d n=%0d", first, bounded.num());
        bounded.get(first);
        bounded.get(second);
        $display("fifo=%0d,%0d n=%0d", first, second, bounded.num());
        $display("empty=%0d", bounded.try_get(first));

        source = "mailbox";
        $display("strput=%0d", inbox.try_put(source));
        source = "changed";
        inbox.get(copied);
        $display("str=%s n=%0d", copied, inbox.num());

        original = new();
        original.id = 77;
        $display("objput=%0d", inbox.try_put(original));
        inbox.get(received);
        object_id = received.id;
        $display("objget=%0d same=%0d id=%0d n=%0d",
            received != null, received == original, object_id, inbox.num());
        inbox.put(99);
        $display("mismatch=%0d n=%0d", inbox.try_get(copied), inbox.num());
        inbox.get(first);
        $display("preserved=%0d n=%0d", first, inbox.num());
        $finish;
    end
endmodule
