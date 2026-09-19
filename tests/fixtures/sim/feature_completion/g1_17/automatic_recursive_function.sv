// IEEE 1800-2009 13.4.2: each automatic activation owns independent storage,
// so a recursive call cannot overwrite the caller's automatic local or return
// slot.
module tb;
    function automatic int weighted_sum(input int n);
        automatic int level;
        level = n * 10;
        if (n <= 0)
            weighted_sum = level;
        else
            weighted_sum = level + weighted_sum(n - 1);
    endfunction

    initial begin
        $display("sum=%0d", weighted_sum(3));
        $finish(0);
    end
endmodule
