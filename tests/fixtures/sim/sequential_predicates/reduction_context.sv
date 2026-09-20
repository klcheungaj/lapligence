module tb;
    logic [7:0] values [0:2];
    logic enabled;
    int total;
    function automatic int sum_enabled(input logic gate);
        return values.sum(row) with (gate &&& row[0] ? int'(row) : 0);
    endfunction
    initial begin
        values = '{8'd1, 8'd2, 8'd3};
        enabled = 1;
        total = sum_enabled(enabled);
        if (total != 4) $fatal(1, "lexical iterator and automatic capture");
        enabled = 0;
        total = sum_enabled(enabled);
        if (total != 0) $fatal(1, "reduction predicate false");
        $display("reduction_context=pass");
        $finish(0);
    end
endmodule
