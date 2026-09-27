// llg-test-fixture: tests/fixtures/sim/audit_a1_packed_constant_patterns/contexts.sv
// IEEE 1800-2009 7.2.1, 7.3.1, 12.5.1, 12.6: whole packed values
// and nested constant patterns retain width, case mode, and member state.
module tb;
    typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pair_t;
    typedef struct packed signed { logic [3:0] hi; logic [3:0] lo; } signed_pair_t;
    typedef struct packed { pair_t inner; logic [7:0] tail; } outer_t;
    typedef struct packed { logic [7:0] high; logic [63:0] low; } wide_t;
    typedef struct packed { logic [3:0] four; bit [3:0] two; } mixed_t;
    typedef union tagged packed { void empty; pair_t data; } tagged_t;
    pair_t pair;
    signed_pair_t signed_pair;
    outer_t outer_value;
    tagged_t tagged_value;
    wide_t wide_value;
    mixed_t mixed_value;
    logic [7:0] raw;
    logic [71:0] wide_raw;
    int result;
    initial begin
        if (!$value$plusargs("v=%h", raw)) raw = 8'ha5;
        pair = pair_t'(raw);
        signed_pair = signed_pair_t'(raw);
        outer_value = '{inner: pair, tail: 8'h3c};
        tagged_value = tagged data pair;
        if (!$value$plusargs("w=%h", wide_raw))
            wide_raw = 72'h12_3456789abcdef012;
        wide_value = wide_t'(wide_raw);
        mixed_value = 'x;

        if (outer_value matches '{inner: pair_t'(8'ha5), tail: 8'h3c}) result = 1;
        else result = 0;
        $display("nested=%0d", result);
        if (tagged_value matches tagged data pair_t'(8'ha5)) result = 1;
        else result = 0;
        $display("tagged=%0d", result);
        if (wide_value matches 72'h12_3456789abcdef012) result = 1;
        else result = 0;
        $display("wide=%0d", result);
        if (signed_pair matches 16'shffa5) result = 1;
        else result = 0;
        $display("signed=%0d", result);
        case (pair) matches
            8'ha5: result = 1;
            default: result = 0;
        endcase
        $display("case=%0d", result);
        casez (pair) matches
            8'ha5: result = 1;
            default: result = 0;
        endcase
        $display("casez=%0d", result);
        casex (pair) matches
            8'ha5: result = 1;
            default: result = 0;
        endcase
        $display("casex=%0d", result);

        if (mixed_value matches '{two: 4'h0, four: 4'hx}) result = 1;
        else result = 0;
        $display("mixed=%0d", result);
        $finish(0);
    end
endmodule
