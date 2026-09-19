module tb;
    int trigger;
    function automatic int helper(input int x);
        int n;
        int total;
        total = x;
        for (n = 0; n < 5; n = n + 1) begin
            if (n == 1) continue;
            if (n == 4) break;
            total = total + n;
        end
        do begin
            total = total + 1;
            n = n - 1;
        end while (n > 2);
        if (x < 0) return 0;
        return total;
    endfunction
    function automatic int outer(input int x);
        return helper(x) + helper(x + 1);
    endfunction
    initial begin
        trigger = 0;
        @(outer(trigger) + outer(trigger + 1));
        if (outer(trigger) != 19) $fatal(1, "callback loop result");
        if (helper(-1) != 0) $fatal(1, "early return");
        $display("callback loops passed");
        $finish(0);
    end
    initial begin
        #1 trigger = 2;
        #5 $fatal(1, "callback never woke");
    end
endmodule
