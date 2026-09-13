// llg-test-fixture: tests/fixtures/sim/mailboxes/locals.sv
// IEEE 1800-2009 §15.4 and Annex G.4: automatic mailbox storage in a nested
// procedural block and in a delay-free task activation.
module tb;
    task automatic task_mailbox(input int seed);
        mailbox #(int) local_box = new(1);
        int value;
        local_box.put(seed);
        local_box.get(value);
        $display("task=%0d n=%0d", value, local_box.num());
    endtask

    initial begin
        int value;
        begin : nested_mailbox
            mailbox #(int) local_box = new(1);
            local_box.put(12);
            local_box.get(value);
            $display("nested=%0d n=%0d", value, local_box.num());
        end
        task_mailbox(21);
        task_mailbox(34);
        $finish;
    end
endmodule
