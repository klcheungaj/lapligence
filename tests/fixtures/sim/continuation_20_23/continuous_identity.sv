// llg-test-fixture: SYN-006 independent contribution slots and complete RHS-only reads.
module tb;
    typedef logic [64:0] row_t[2];
    row_t left = '{65'd1, 65'd2};
    row_t right = '{65'd3, 65'd4};
    logic choice = 1;
    wire [64:0] resolved[2];
    row_t selected;
    row_t constant_value;
    assign resolved = left;
    assign resolved = right;
    assign '{resolved[1], resolved[0]} = left;
    assign selected = choice ? left : right;
    assign constant_value = '{65'd9, 65'd10};
    initial begin
        #1;
        if (selected !== left || constant_value[0] !== 65'd9 || constant_value[1] !== 65'd10)
            $fatal(1, "initial continuous graph");
        choice = 0;
        #1;
        if (selected !== right) $fatal(1, "continuous selector");
        right[1] = 65'd8;
        #1;
        if (selected[1] !== 65'd8) $fatal(1, "continuous content");
        $display("CONTINUOUS_IDENTITY_PASS");
        $finish(0);
    end
endmodule
