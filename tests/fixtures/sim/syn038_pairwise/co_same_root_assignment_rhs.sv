// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/co_same_root_assignment_rhs.sv
// IEEE 1800-2009 §§6.5, 6.8, 7.2, 10.4 and 10.5.
// Every assignment RHS reads the same declared object that its LHS writes.
module tb;
    typedef struct {
        logic [7:0] value;
        logic [7:0] guard;
    } record_t;
    typedef logic [1:0] bits_t;

    logic [7:0] whole = 8'h10;
    record_t record = '{value: 8'h20, guard: 8'h55};
    logic [7:0] concat_value = 8'h96;
    bits_t pattern_value = 2'b10;
    logic [7:0] nba_value = 8'h30;

    initial begin
        if (whole !== 8'h10 || record.value !== 8'h20 || record.guard !== 8'h55 ||
            concat_value !== 8'h96 || pattern_value !== 2'b10 || nba_value !== 8'h30)
            $fatal(1, "same-root initial readback");

        whole = whole + 8'h01;
        if (whole !== 8'h11)
            $fatal(1, "same-root whole-object blocking readback");

        record.value = record.value + 8'h01;
        if (record.value !== 8'h21 || record.guard !== 8'h55)
            $fatal(1, "same-root field blocking readback");

        {concat_value[3:0], concat_value[7:4]} = concat_value;
        if (concat_value !== 8'h69)
            $fatal(1, "same-root concatenation blocking readback");

        bits_t'{pattern_value[1], pattern_value[0]} = {pattern_value[0], pattern_value[1]};
        if (pattern_value !== 2'b01)
            $fatal(1, "same-root positional-pattern blocking readback");

        nba_value <= nba_value + 8'h01;
        if (nba_value !== 8'h30)
            $fatal(1, "same-root NBA pre-commit readback");
        #1;
        if (nba_value !== 8'h31)
            $fatal(1, "same-root NBA post-commit readback");

        $display("same_root=%h,%h,%h,%b,%h", whole, record.value, concat_value,
            pattern_value, nba_value);
        $finish(0);
    end
endmodule
