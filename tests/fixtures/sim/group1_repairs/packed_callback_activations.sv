module tb;
    typedef struct packed { logic [7:0] hi; logic [7:0] lo; } pair_t;
    pair_t first = 16'h0102;
    pair_t second = 16'h0304;
    int changes = 0;
    int private_changes = 0;
    function automatic int leaf(input pair_t x);
        return int'(x.hi) + int'(x.lo);
    endfunction
    function automatic int nested(input pair_t x, input pair_t y);
        return leaf(x) + leaf(y);
    endfunction
    function automatic int private_mutation(input pair_t x);
        x.lo++;
        return leaf(x);
    endfunction
    always @(nested(first, second) + nested(second, first)) changes++;
    always @(private_mutation(first) + private_mutation(first)) private_changes++;
    initial begin
        #1; first = 16'h0103;
        #1; second = 16'h0305;
        #1;
        if (changes != 2 || private_changes != 1) $fatal(1, "packed callback reevaluation");
        if (first !== 16'h0103 || second !== 16'h0305) $fatal(1, "callback input changed caller");
        $display("packed callbacks passed");
        $finish(0);
    end
endmodule
