module tb;
    real trigger;
    function automatic real half(input real x);
        if (x < 0.0) return 0.0;
        return x + 0.5;
    endfunction
    function automatic shortreal quarter(input shortreal x);
        return x + 0.25;
    endfunction
    function automatic real outer(input real x);
        return half(half(x)) + quarter(1.0);
    endfunction
    initial begin
        trigger = 0.0;
        @(outer(trigger) + half(trigger));
        if (outer(trigger) != 4.25) $fatal(1, "real callback result");
        if (half(-1.0) != 0.0) $fatal(1, "real early return");
        $display("real callbacks passed");
        $finish(0);
    end
    initial begin
        #1 trigger = 2.0;
        #5 $fatal(1, "real callback never woke");
    end
endmodule
