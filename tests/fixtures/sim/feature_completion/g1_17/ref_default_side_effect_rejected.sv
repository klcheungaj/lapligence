// IEEE 1800-2009 13.5.2: a default argument that references an earlier formal
// reads that formal's value. The earlier actual here calls a side-effecting
// function, so reusing the argument expression for the default would evaluate
// it twice; the simulator must not silently miscompile that case.
module tb;
    int calls = 0;

    function automatic int next_default();
        calls = calls + 1;
        next_default = calls;
    endfunction

    task automatic t(input int a = next_default(),
                     input int b = a + 1,
                     output int y);
        y = a + b;
    endtask

    int y;
    initial begin
        t(, , y);
        $display("calls=%0d y=%0d", calls, y);
        $finish(0);
    end
endmodule
