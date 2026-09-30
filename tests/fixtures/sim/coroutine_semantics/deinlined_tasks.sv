// llg-test-fixture: tests/fixtures/sim/coroutine_semantics/deinlined_tasks.sv
// Tasks with event formals, event controls or a `disable` are ordinary calls:
// an event formal names the caller's event object, a chain of event tasks
// keeps each activation's locals and outputs, disabling a suspended task
// skips its copy-out while the caller continues, and a timed task can disable
// its caller's block.
module tb;
    event first;
    event second;
    event chain_tick;
    event slow_tick;
    event block_tick;

    integer first_seen = 0;
    integer second_seen = 0;
    integer chain_a = 0;
    integer chain_b = 0;
    integer chain_c = 0;
    integer slow_a = 100;
    integer slow_b = 200;
    integer slow_after_a = 0;
    integer slow_after_b = 0;
    integer slow_fresh = 0;
    integer timed_result = 5;
    integer timed_after_call = 0;
    integer timed_after_block = 0;
    integer event_result = 6;
    integer event_after_call = 0;
    integer event_after_block = 0;

    // An event formal is the caller's event object: waiting and triggering
    // through it are visible to the caller and to every other bound.
    task automatic wait_event(input event e, input integer tag, output integer seen);
        @(e);
        #1;
        seen = tag;
    endtask

    task automatic fire(input event e);
        -> e;
    endtask

    // Four event levels, each with a live local and an output, entered from
    // three call sites.
    task automatic level4(input event e, input integer tag, output integer result);
        automatic integer local_tag = tag;
        @(e);
        if (local_tag != tag) $fatal(1, "level4 local");
        result = tag + 1;
    endtask
    task automatic level3(input event e, input integer tag, output integer result);
        automatic integer local_tag = tag; integer child;
        @(e);
        level4(e, tag, child);
        if (local_tag != tag) $fatal(1, "level3 local");
        result = child + 1;
    endtask
    task automatic level2(input event e, input integer tag, output integer result);
        automatic integer local_tag = tag; integer child;
        @(e);
        level3(e, tag, child);
        if (local_tag != tag) $fatal(1, "level2 local");
        result = child + 1;
    endtask
    task automatic level1(input event e, input integer tag, output integer result);
        automatic integer local_tag = tag; integer child;
        @(e);
        level2(e, tag, child);
        if (local_tag != tag) $fatal(1, "level1 local");
        result = child + 1;
    endtask

    // A timed task with an event control that another process disables while
    // it waits: its output is not copied back.
    task automatic slow(input event e, output integer result, input integer value);
        result = value;
        @(e);
        result = value + 1;
    endtask

    // A timed task that disables its caller's block, with and without an
    // event control.
    task automatic stop_timed(output integer result);
        result = 9;
        #1;
        disable timed_block;
        result = 10;
    endtask
    task automatic stop_on_event(input event e, output integer result);
        result = 9;
        @(e);
        disable event_block;
        result = 10;
    endtask

    event bound;

    // Identity: the waiter binds the object `bound` names at the call; a later
    // rebinding of the caller's handle does not move it.
    initial begin
        bound = first;
        wait_event(bound, 1, first_seen);
    end
    initial begin
        #1;
        bound = second;
        wait_event(bound, 2, second_seen);
    end
    initial begin
        #3 fire(first);
        #3 fire(second);
        #1 -> first;
        -> second;
    end

    initial begin
        #10;
        if (first_seen != 1 || second_seen != 2)
            $fatal(1, "event formal identity mismatch %0d %0d", first_seen, second_seen);
    end

    // Chain: three call sites, four triggers.
    initial level1(chain_tick, 100, chain_a);
    initial level1(chain_tick, 200, chain_b);
    initial begin
        #1;
        level1(chain_tick, 300, chain_c);
    end
    initial begin
        #20 -> chain_tick;
        #1 -> chain_tick;
        #1 -> chain_tick;
        #1 -> chain_tick;
        #1;
        if (chain_a != 104 || chain_b != 204 || chain_c != 304)
            $fatal(1, "chain mismatch %0d %0d %0d", chain_a, chain_b, chain_c);
    end

    // Disable the task while two invocations wait; the callers continue.
    initial begin
        slow(slow_tick, slow_a, 10);
        slow_after_a = 1;
    end
    initial begin
        slow(slow_tick, slow_b, 20);
        slow_after_b = 1;
    end
    initial begin
        #30 disable slow;
        #1;
        if (slow_a != 100 || slow_b != 200)
            $fatal(1, "disabled task copied out %0d %0d", slow_a, slow_b);
        if (slow_after_a != 1 || slow_after_b != 1)
            $fatal(1, "callers did not continue");
        fork
            slow(slow_tick, slow_a, 30);
        join_none
        #1 -> slow_tick;
        #1;
        if (slow_a != 31) $fatal(1, "fresh call after disable %0d", slow_a);
        slow_fresh = 1;
    end

    // A timed task disables its caller's block.
    initial begin
        begin : timed_block
            stop_timed(timed_result);
            timed_after_call = 1;
        end
        timed_after_block = 1;
    end
    initial begin
        begin : event_block
            stop_on_event(block_tick, event_result);
            event_after_call = 1;
        end
        event_after_block = 1;
    end
    initial #40 -> block_tick;

    initial begin
        #50;
        if (timed_result != 5 || timed_after_call != 0 || timed_after_block != 1)
            $fatal(1, "timed callee disable mismatch");
        if (event_result != 6 || event_after_call != 0 || event_after_block != 1)
            $fatal(1, "event callee disable mismatch");
        $display("PASS deinlined_tasks first=%0d second=%0d chain=%0d/%0d/%0d slow=%0d/%0d fresh=%0d timed=%0d event=%0d",
            first_seen, second_seen, chain_a, chain_b, chain_c, slow_a, slow_b,
            slow_fresh, timed_after_block, event_after_block);
        $finish(0);
    end
endmodule
