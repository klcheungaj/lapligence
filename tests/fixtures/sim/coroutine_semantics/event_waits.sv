// llg-test-fixture: tests/fixtures/sim/coroutine_semantics/event_waits.sv
// IEEE 1800-2009 §§15.5.3 and 15.5.4: an event OR-list wakes once, event
// aliases preserve identity and triggered state lasts through the time slot,
// and wait_order reports both success and out-of-order failure.
module tb;
    event left;
    event right;
    event alias_event;
    event first;
    event second;
    integer wakes = 0;
    integer triggered_checks = 0;
    integer successes = 0;
    integer failures = 0;

    initial begin
        alias_event = left;
        fork
            begin
                @(alias_event or right);
                wakes = wakes + 1;
                @(alias_event or right);
                wakes = wakes + 1;
            end
            begin
                wait (alias_event.triggered);
                if (alias_event.triggered) triggered_checks++;
                #0;
                if (alias_event.triggered) triggered_checks++;
                #1;
                if (!alias_event.triggered) triggered_checks++;
            end
            begin
                wait_order (first, second) successes++;
                else failures++;
                wait_order (first, second) successes++;
                else failures++;
            end
            begin
                #1 -> left;
                -> right;
                #1 -> right;
                -> first;
                #0 -> first;
                #1 -> second;
                #1 -> second;
            end
        join
        if (wakes != 2 || triggered_checks != 3 ||
            successes != 1 || failures != 1)
            $fatal(1, "event oracle mismatch");
        $display("PASS event_waits wakes=%0d triggered=%0d success=%0d failure=%0d",
            wakes, triggered_checks, successes, failures);
        $finish(0);
    end
endmodule
