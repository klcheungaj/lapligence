module tb;
    typedef struct packed { logic [7:0] hi; logic [7:0] lo; } pair_t;
    pair_t original, first, second;
    function automatic pair_t descend(input pair_t copy, input int n);
        if (n == 0) return copy;
        copy.lo += 8'(n);
        descend = descend(copy, n - 1);
        if (copy.hi !== 8'h11) $fatal(1, "recursive input owner was overwritten");
    endfunction
    initial begin
        original = 16'h1101;
        first = descend(original, 3);
        second = descend(original, 2);
        if (first !== 16'h1107 || second !== 16'h1104 || original !== 16'h1101)
            $fatal(1, "packed recursive return/input ownership");
        $display("packed recursion passed");
        $finish(0);
    end
endmodule
