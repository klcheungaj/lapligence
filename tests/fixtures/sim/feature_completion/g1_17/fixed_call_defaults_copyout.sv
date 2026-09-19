// IEEE 1800-2009 13.5.2/13.5.3: a default argument is evaluated only when the
// call omits it, in the callee's declaration scope; output and inout formals
// copy back once, after the call returns.
module tb;
    int default_calls = 0;
    int shared;

    function automatic int next_default();
        default_calls = default_calls + 1;
        next_default = 40 + default_calls;
    endfunction

    task automatic take_default(input int a = next_default(), output int y);
        y = a;
    endtask

    task automatic combine(input int a = 7,
                           input int b = a + 1,
                           output int sum,
                           inout int acc);
        sum = a + b;
        acc = acc + a;
    endtask

    task automatic deferred_out(output int o);
        $display("inside=%0d", shared);
        o = 99;
        $display("inside2=%0d", shared);
    endtask

    int y;
    int s;
    int c;
    initial begin
        take_default(, y);
        take_default(5, y);
        $display("defaults=%0d y=%0d", default_calls, y);
        c = 100;
        combine(, , s, c);
        $display("defaults=%0d s=%0d c=%0d", default_calls, s, c);
        combine(5, 6, s, c);
        $display("defaults=%0d s=%0d c=%0d", default_calls, s, c);
        shared = 7;
        deferred_out(shared);
        $display("after=%0d", shared);
        $finish(0);
    end
endmodule
