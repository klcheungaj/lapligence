// llg-test-fixture: tests/fixtures/sim/coroutine_semantics/ref_event_tasks.sv
// A `ref` formal read by an event control keeps the actual's dependencies
// through a chain of forwarding tasks, for several module signals and a caller
// local, and a disabled task never observes a later edge.
module tb;
    logic tick_a = 0;
    logic tick_b = 0;
    integer sum_a = 0;
    integer sum_b = 0;
    integer sum_local = 0;
    integer sum_killed = 0;
    integer after_killed = 0;

    task automatic level1(inout integer sum, ref logic source, input integer value);
        @(posedge source) sum = sum + value;
    endtask
    task automatic level2(inout integer sum, ref logic source, input integer value);
        level1(sum, source, value);
        @(posedge source) sum = sum + 2 * value;
    endtask
    task automatic level3(inout integer sum, ref logic source, input integer value);
        level2(sum, source, value);
        @(negedge source) sum = sum + 4 * value;
    endtask

    task automatic killable(ref logic source);
        @(posedge source) sum_killed = sum_killed + 1;
    endtask

    // Two module signals, each with its own chain.
    initial level3(sum_a, tick_a, 1);
    initial level3(sum_b, tick_b, 10);

    // A caller local is not a module signal; nothing else can change it, so
    // it never wakes the task and its output stays untouched.
    initial begin
        automatic logic local_tick = 0;
        fork
            level1(sum_local, local_tick, 1000);
        join_none
        #30;
        if (sum_local != 0) $fatal(1, "local actual woke");
    end

    initial begin
        fork
            begin killable(tick_b); after_killed = 1; end
        join_none
        #2 disable killable;
    end

    initial begin
        #1 tick_a = 1;
        #1 tick_a = 0;
        #1 tick_a = 1;
        #1 tick_a = 0;
        #1 tick_b = 1;
        #1 tick_b = 0;
        #1 tick_b = 1;
        #1 tick_b = 0;
        #1 tick_a = 1;
        #1;
        if (sum_a != 7 || sum_b != 70 || sum_killed != 0 || after_killed != 1)
            $fatal(1, "chain sums %0d %0d killed %0d/%0d", sum_a, sum_b, sum_killed,
                after_killed);
        $display("PASS ref_event_tasks a=%0d b=%0d local=%0d killed=%0d/%0d",
            sum_a, sum_b, sum_local, sum_killed, after_killed);
        $finish(0);
    end
endmodule
