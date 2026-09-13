// llg-test-fixture: tests/fixtures/sim/mailboxes/blocking.sv
// IEEE 1800-2009 §15.4 and Annex G.4: blocking producer/consumer FIFO queues,
// multiple peek/get waiters, and bounded queue handoff.
module tb;
    mailbox #(int) bounded = new(1);
    mailbox #(int) observed = new(0);
    int a;
    int b;
    int c;
    int p;
    int q;
    int r;

    initial begin
        bounded.put(1);
        fork
            begin
                bounded.put(2);
                $display("producer2 n=%0d", bounded.num());
            end
            begin
                #0 bounded.put(3);
                $display("producer3 n=%0d", bounded.num());
            end
            begin
                #1 bounded.get(a);
                $display("consumer1=%0d n=%0d", a, bounded.num());
                #1 bounded.get(b);
                $display("consumer2=%0d n=%0d", b, bounded.num());
                #1 bounded.get(c);
                $display("consumer3=%0d n=%0d", c, bounded.num());
            end
        join

        fork
            begin
                observed.peek(p);
                $display("peek1=%0d n=%0d", p, observed.num());
            end
            begin
                #0 observed.peek(q);
                $display("peek2=%0d n=%0d", q, observed.num());
            end
            begin
                #0 observed.get(r);
                $display("get=%0d n=%0d", r, observed.num());
            end
            begin
                #1 observed.put(8);
            end
        join
        $display("observed_final=%0d", observed.num());
        $finish;
    end
endmodule
