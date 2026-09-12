// llg-test-fixture: tests/fixtures/sim/partial_features/event_h15.sv
// IEEE 1800-2009 §§15.5.1–15.5.4: event-handle aliases and null rebinding,
// same-slot triggered state, queued target identity, and ordered waits.
module tb;
    event first;
    event second;
    event handle;
    event null_handle;
    event stale_handle;
    event queued_handle;
    event trigger_event;
    event triggered_handle;

    integer output_wakes = 0;
    integer inout_wakes = 0;
    integer null_wakes = 0;
    integer stale_wakes = 0;
    integer new_wakes = 0;
    integer queued_wakes = 0;
    integer ordered_successes = 0;
    integer ordered_failures = 0;
    integer triggered_wait = 0;
    integer triggered_delta = 0;
    integer triggered_reset = 0;
    integer ordinary_latched = 0;

    task automatic output_alias(output event e);
        e = first;
    endtask

    task automatic inout_alias(inout event e);
        e = second;
    endtask

    // Set up waiters before the handles are rebound. The #0 is also the
    // suspension boundary that starts all join_none children in this slot.
    initial begin
        output_alias(handle);
        fork
            begin
                @(handle);
                output_wakes = output_wakes + 1;
            end
        join_none

        fork
            begin
                @(null_handle);
                null_wakes = null_wakes + 1;
            end
        join_none

        stale_handle = first;
        fork
            begin
                @(stale_handle);
                stale_wakes = stale_wakes + 1;
            end
        join_none

        queued_handle = first;
        fork
            begin
                @(first);
                queued_wakes = queued_wakes + 1;
            end
        join_none

        triggered_handle = trigger_event;
        #0;

        // Rebinding does not migrate a waiter already attached to the old
        // object. The new child, started by the following wait_order, sees the
        // new object instead.
        inout_alias(handle);
        fork
            begin
                @(handle);
                inout_wakes = inout_wakes + 1;
            end
        join_none
        null_handle = null;
        null_handle = first;
        stale_handle = second;
        fork
            begin
                @(stale_handle);
                new_wakes = new_wakes + 1;
            end
        join_none

        wait_order (first, second) begin
            ordered_successes = ordered_successes + 1;
        end else begin
            ordered_failures = ordered_failures + 1;
        end
        wait_order (first, second) begin
            ordered_successes = ordered_successes + 1;
        end else begin
            ordered_failures = ordered_failures + 1;
        end
    end

    // The queued trigger captures first before the handle is rebound. The
    // second first-event trigger is issued in the same slot and is ignored by
    // wait_order as an already-consumed event.
    initial begin
        #1 ->> queued_handle;
        queued_handle = second;
        #0;
        ->> first;
        #1 -> second;
        #1 -> second;
    end

    // Spawn the triggered waiter only after the trigger. It must still observe
    // the persistent state in this time slot, including across #0, then clear
    // when time advances.
    initial begin
        #1 -> trigger_event;
        fork
            begin
                wait (triggered_handle.triggered);
                triggered_wait = triggered_wait + 1;
                #0;
                if (triggered_handle.triggered)
                    triggered_delta = triggered_delta + 1;
                #1;
                if (!triggered_handle.triggered)
                    triggered_reset = triggered_reset + 1;
            end
            begin
                @(trigger_event);
                ordinary_latched = ordinary_latched + 1;
            end
        join_none
        #4;
    end

    initial begin
        #4;
        $display("aliases=%0d/%0d null=%0d stale=%0d/%0d queued=%0d order=%0d/%0d triggered=%0d/%0d/%0d ordinary=%0d",
            output_wakes, inout_wakes, null_wakes, stale_wakes, new_wakes,
            queued_wakes, ordered_successes, ordered_failures, triggered_wait,
            triggered_delta, triggered_reset, ordinary_latched);
        $finish(0);
    end
endmodule
