// llg-test-fixture: SYN-006 one reached function value per driver evaluation, not per cell.
module tb;
    typedef logic [64:0] row_t[2];
    row_t source = '{65'd1, 65'd2};
    wire [64:0] output_value[2];
    wire [64:0] pattern_value[2];
    function automatic row_t produce(input row_t value);
        $display("CONTINUOUS_RHS_EVAL");
        return value;
    endfunction
    assign output_value = produce(source);
    assign '{pattern_value[1], pattern_value[0]} = source;
    initial begin
        #1;
        if (output_value !== source || pattern_value[1] !== source[0] || pattern_value[0] !== source[1])
            $fatal(1, "initial RHS snapshot or pattern topology");
        source[0] = 65'd3;
        #1;
        if (output_value !== source || pattern_value[1] !== source[0]) $fatal(1, "first content update");
        source[1] = 65'd4;
        #1;
        if (output_value !== source || pattern_value[0] !== source[1]) $fatal(1, "second content update");
        $display("CONTINUOUS_RHS_ONCE_PASS");
        $finish(0);
    end
endmodule
