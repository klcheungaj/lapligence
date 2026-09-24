// llg-test-fixture: tests/fixtures/sim/review_bundle/r12_record_conditional_2state_nba.sv
// IEEE 1800-2009 §§7.2, 10.4, 11.4.11: merge an unknown conditional selector
// into an unpacked record containing a two-state member through an NBA.
module tb;
    typedef struct {
        logic [7:0] data;
        bit valid;
    } record_t;

    record_t left_value;
    record_t right_value;
    record_t result;
    logic selector;

    initial begin
        left_value.data = 8'h00;
        left_value.valid = 1'b0;
        right_value.data = 8'hff;
        right_value.valid = 1'b1;
        result.data = 8'h55;
        result.valid = 1'b1;
        selector = 1'bx;

        result <= selector ? left_value : right_value;
        if (result.data !== 8'h55 || result.valid !== 1'b1)
            $fatal(1, "record NBA committed before the NBA region");

        #1;
        if (result.data !== 8'hxx || result.valid !== 1'b0)
            $fatal(1, "conditional merge lost record member semantics");
        $display("record_nba=%h,%b", result.data, result.valid);
        $finish(0);
    end
endmodule
