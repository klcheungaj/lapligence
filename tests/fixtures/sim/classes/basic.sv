// llg-test-fixture: tests/fixtures/sim/classes/basic.sv
// IEEE 1800-2009 §§8.3–8.10: nominal class construction, property defaults,
// constructors, static state, this-bound methods/tasks, and handle aliasing.
class Counter;
    int count = 1;
    int next = count + 1;
    real ratio = 1.5;
    static int total = 0;

    function new(int seed = 2);
        count = seed;
        next = next + 1;
        total = total + 1;
    endfunction

    function int value();
        value = count + next * 10;
    endfunction

    function int twice_value();
        twice_value = value() + value();
    endfunction

    function void increment();
        count = count + 1;
    endfunction

    function int add(input int delta);
        int temporary;
        temporary = count + delta;
        count = temporary;
        add = temporary;
    endfunction

    function real ratio_value();
        ratio_value = ratio;
    endfunction

    task automatic bump_task(input int delta);
        int temporary;
        temporary = count + delta;
        count = temporary;
    endtask

    static function int total_value();
        total_value = total;
    endfunction
endclass

module tb;
    Counter first = new();
    Counter second;
    Counter copy;

    initial begin
        second = new(7);
        copy = first;
        first.increment();
        second.bump_task(1);
        $display("first=%0d alias=%0d second=%0d twice=%0d ratio=%f",
            first.value(), copy.value(), second.value(), first.twice_value(), first.ratio_value());
        $display("add=%0d", first.add(2));
        $display("first_after=%0d alias_after=%0d total=%0d",
            first.value(), copy.value(), Counter::total_value());
        $finish;
    end
endmodule
