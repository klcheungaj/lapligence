// llg-test-fixture: tests/fixtures/sim/coroutine_semantics/nested_timing_calls.sv
// Delay-only and event-bearing timing tasks are both ordinary timed calls.
// Calls at depths one through eight make every activation suspend twice and
// check its automatic local after wakeup.
module tb;
    event tick;
    integer direct_calls = 0;

    task automatic plain(input integer value, output integer result);
        direct_calls++;
        result = value + 1;
    endtask

    task automatic d8(input integer value, output integer result);
        automatic integer local_value = value;
        #1; if (local_value != value) $fatal(1, "d8 first resume");
        local_value++;
        #1; if (local_value != value + 1) $fatal(1, "d8 second resume");
        result = local_value + 1;
    endtask
    task automatic d7(input integer value, output integer result);
        automatic integer local_value = value; integer child;
        #1; if (local_value != value) $fatal(1, "d7 first resume");
        d8(local_value, child);
        #1; if (local_value != value || child != value + 2) $fatal(1, "d7 second resume");
        result = child + 1;
    endtask
    task automatic d6(input integer value, output integer result);
        automatic integer local_value = value; integer child;
        #1; if (local_value != value) $fatal(1, "d6 first resume"); d7(local_value, child);
        #1; if (local_value != value || child != value + 3) $fatal(1, "d6 second resume"); result = child + 1;
    endtask
    task automatic d5(input integer value, output integer result);
        automatic integer local_value = value; integer child;
        #1; if (local_value != value) $fatal(1, "d5 first resume"); d6(local_value, child);
        #1; if (local_value != value || child != value + 4) $fatal(1, "d5 second resume"); result = child + 1;
    endtask
    task automatic d4(input integer value, output integer result);
        automatic integer local_value = value; integer child;
        #1; if (local_value != value) $fatal(1, "d4 first resume"); d5(local_value, child);
        #1; if (local_value != value || child != value + 5) $fatal(1, "d4 second resume"); result = child + 1;
    endtask
    task automatic d3(input integer value, output integer result);
        automatic integer local_value = value; integer child;
        #1; if (local_value != value) $fatal(1, "d3 first resume"); d4(local_value, child);
        #1; if (local_value != value || child != value + 6) $fatal(1, "d3 second resume"); result = child + 1;
    endtask
    task automatic d2(input integer value, output integer result);
        automatic integer local_value = value; integer child;
        #1; if (local_value != value) $fatal(1, "d2 first resume"); d3(local_value, child);
        #1; if (local_value != value || child != value + 7) $fatal(1, "d2 second resume"); result = child + 1;
    endtask
    task automatic d1(input integer value, output integer result);
        automatic integer local_value = value; integer child;
        #1; if (local_value != value) $fatal(1, "d1 first resume"); d2(local_value, child);
        #1; if (local_value != value || child != value + 8) $fatal(1, "d1 second resume"); result = child + 1;
    endtask

    task automatic e8(input integer value, output integer result);
        automatic integer local_value = value;
        @(tick); if (local_value != value) $fatal(1, "e8 first resume");
        local_value++;
        @(tick); if (local_value != value + 1) $fatal(1, "e8 second resume");
        result = local_value + 1;
    endtask
    task automatic e7(input integer value, output integer result);
        automatic integer local_value = value; integer child;
        @(tick); if (local_value != value) $fatal(1, "e7 first resume"); e8(local_value, child);
        @(tick); if (local_value != value || child != value + 2) $fatal(1, "e7 second resume"); result = child + 1;
    endtask
    task automatic e6(input integer value, output integer result);
        automatic integer local_value = value; integer child;
        @(tick); if (local_value != value) $fatal(1, "e6 first resume"); e7(local_value, child);
        @(tick); if (local_value != value || child != value + 3) $fatal(1, "e6 second resume"); result = child + 1;
    endtask
    task automatic e5(input integer value, output integer result);
        automatic integer local_value = value; integer child;
        @(tick); if (local_value != value) $fatal(1, "e5 first resume"); e6(local_value, child);
        @(tick); if (local_value != value || child != value + 4) $fatal(1, "e5 second resume"); result = child + 1;
    endtask
    task automatic e4(input integer value, output integer result);
        automatic integer local_value = value; integer child;
        @(tick); if (local_value != value) $fatal(1, "e4 first resume"); e5(local_value, child);
        @(tick); if (local_value != value || child != value + 5) $fatal(1, "e4 second resume"); result = child + 1;
    endtask
    task automatic e3(input integer value, output integer result);
        automatic integer local_value = value; integer child;
        @(tick); if (local_value != value) $fatal(1, "e3 first resume"); e4(local_value, child);
        @(tick); if (local_value != value || child != value + 6) $fatal(1, "e3 second resume"); result = child + 1;
    endtask
    task automatic e2(input integer value, output integer result);
        automatic integer local_value = value; integer child;
        @(tick); if (local_value != value) $fatal(1, "e2 first resume"); e3(local_value, child);
        @(tick); if (local_value != value || child != value + 7) $fatal(1, "e2 second resume"); result = child + 1;
    endtask
    task automatic e1(input integer value, output integer result);
        automatic integer local_value = value; integer child;
        @(tick); if (local_value != value) $fatal(1, "e1 first resume"); e2(local_value, child);
        @(tick); if (local_value != value || child != value + 8) $fatal(1, "e1 second resume"); result = child + 1;
    endtask

    initial forever begin #1 -> tick; end

    initial begin
        integer result;
        integer shallow;
        plain(0, result);
        if (result != 1) $fatal(1, "plain first call");

        d8(10, result); if (result != 12) $fatal(1, "delay depth one");
        d7(10, result); if (result != 13) $fatal(1, "delay depth two");
        d6(10, result); if (result != 14) $fatal(1, "delay depth three");
        d5(10, result); if (result != 15) $fatal(1, "delay depth four");
        d4(10, result); if (result != 16) $fatal(1, "delay depth five");
        d3(10, result); if (result != 17) $fatal(1, "delay depth six");
        d2(10, result); if (result != 18) $fatal(1, "delay depth seven");
        d1(35, result); if (result != 44) $fatal(1, "delay depth eight");
        d8(20, shallow); if (shallow != 22) $fatal(1, "delay repeated shallow");

        e8(10, result); if (result != 12) $fatal(1, "event depth one");
        e7(10, result); if (result != 13) $fatal(1, "event depth two");
        e6(10, result); if (result != 14) $fatal(1, "event depth three");
        e5(10, result); if (result != 15) $fatal(1, "event depth four");
        e4(10, result); if (result != 16) $fatal(1, "event depth five");
        e3(10, result); if (result != 17) $fatal(1, "event depth six");
        e2(10, result); if (result != 18) $fatal(1, "event depth seven");
        e1(35, result); if (result != 44) $fatal(1, "event depth eight");
        e8(20, shallow); if (shallow != 22) $fatal(1, "event repeated shallow");

        plain(1, result);
        if (result != 2 || direct_calls != 2) $fatal(1, "plain second call");
        $display("PASS nested_timing_calls delay=44 event=44 direct=%0d", direct_calls);
        $finish(0);
    end
endmodule
