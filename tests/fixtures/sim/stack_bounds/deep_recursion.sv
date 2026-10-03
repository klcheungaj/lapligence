// SystemVerilog recursion must not consume native stack per level: recursive
// subprograms run as stackless coroutines whose recursive calls use the chain
// arena. Every recursion here is 200-250 levels deep (below the 256-call
// guard): a long-bodied function, mutual recursion, a void function with an
// output, a task without timing, string and real results, a recursive
// constructor, and recursion through class virtual dispatch and through a
// virtual-interface method call.
interface counter_if;
    int base = 0;
    function automatic int down(virtual counter_if handle, int n);
        if (n == 0) return base;
        return 1 + handle.down(handle, n - 1);
    endfunction
endinterface

class Walker;
    virtual counter_if handle;
    function new(virtual counter_if initial_handle);
        handle = initial_handle;
    endfunction
    function int run(int n);
        return handle.down(handle, n);
    endfunction
endclass

class Node;
    Node next;
    int value;
    function new(int depth);
        value = depth;
        if (depth > 0) next = new(depth - 1);
    endfunction
    virtual function int sum();
        return value + (next == null ? 0 : next.sum());
    endfunction
endclass

class Doubler extends Node;
    function new(int depth);
        super.new(depth);
    endfunction
    virtual function int sum();
        return 2 * value + (next == null ? 0 : next.sum());
    endfunction
endclass

module tb;
    function automatic longint unsigned mix(input integer n, input longint unsigned a);
        longint unsigned b;
        b = a;
        b = (b + a) ^ (b * 3);  b = b - (a >> 1);
        b = (b + 5) ^ (b * 7);  b = b - (a >> 2);
        b = (b + a) ^ (b * 11); b = b - (a >> 3);
        b = (b + 13) ^ (b * 17); b = b - (a >> 4);
        b = (b + a) ^ (b * 19); b = b - (a >> 5);
        b = (b + 23) ^ (b * 29); b = b - (a >> 6);
        b = (b + a) ^ (b * 31); b = b - (a >> 7);
        b = (b + 37) ^ (b * 41); b = b - (a >> 8);
        if (n == 0) return b;
        return mix(n - 1, b) + 64'd1;
    endfunction

    function automatic bit is_even(input integer n);
        return n == 0 ? 1'b1 : is_odd(n - 1);
    endfunction

    function automatic bit is_odd(input integer n);
        return n == 0 ? 1'b0 : is_even(n - 1);
    endfunction

    function automatic void count_down(input integer n, output integer calls);
        integer inner;
        if (n == 0) begin
            calls = 0;
            return;
        end
        count_down(n - 1, inner);
        calls = inner + 1;
    endfunction

    task automatic walk(input integer n, inout integer total);
        if (n > 0) begin
            total = total + n;
            walk(n - 1, total);
        end
    endtask

    function automatic string repeat_x(input integer n);
        if (n == 0) return "";
        return {repeat_x(n - 1), "x"};
    endfunction

    function automatic real half_sum(input integer n);
        if (n == 0) return 0.5;
        return 1.0 + half_sum(n - 1);
    endfunction

    counter_if counter();
    Walker walker;
    Node list;
    Doubler doubled;
    integer calls;
    integer total;

    initial begin
        counter.base = 7;
        walker = new(counter);
        list = new(200);
        doubled = new(200);
        count_down(250, calls);
        total = 0;
        walk(250, total);
        $display("mix=%h", mix(250, 64'h0123_4567_89ab_cdef));
        $display("even=%0d odd=%0d", is_even(250), is_odd(251));
        $display("calls=%0d total=%0d", calls, total);
        $display("len=%0d half=%0.1f", repeat_x(250).len(), half_sum(250));
        $display("list=%0d doubled=%0d", list.sum(), doubled.sum());
        $display("vif=%0d", walker.run(240));
        $finish(0);
    end
endmodule
