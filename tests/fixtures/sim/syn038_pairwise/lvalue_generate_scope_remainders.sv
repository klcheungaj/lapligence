// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/lvalue_generate_scope_remainders.sv
// Selected procedural lvalue forms in a generated scope.
module tb;
    typedef logic [7:0] byte_t;
    typedef byte_t pair_t [0:1];
    typedef logic [1:0] bits_t;

    pair_t generated_row;
    logic [15:0] generated_concat;
    bits_t generated_pattern;

    if (1'b1) begin : generated_lvalues
        initial begin
            generated_row[0:1] = '{0:8'h31, 1:8'h32};
            {generated_concat[15:8], generated_concat[7:0]} = 16'h4142;
            bits_t'{generated_pattern[1], generated_pattern[0]} = 2'b01;
            if (generated_row[0] !== 8'h31 || generated_row[1] !== 8'h32 ||
                generated_concat !== 16'h4142 || generated_pattern !== 2'b01)
                $fatal(1, "generated lvalue targets");
        end
    end

    initial begin
        #1;
        if (generated_row[0] !== 8'h31 || generated_row[1] !== 8'h32 ||
            generated_concat !== 16'h4142 || generated_pattern !== 2'b01)
            $fatal(1, "generated lvalue readback");
        $display("generated_row=%h,%h generated_concat=%h generated_pattern=%b",
                 generated_row[0], generated_row[1], generated_concat, generated_pattern);
        $finish(0);
    end
endmodule
