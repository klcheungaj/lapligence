// llg-test-fixture: tests/fixtures/sim/syn012_fixed_layout/native_record_rejected.sv
// IEEE 1800-2009 §7.2: this single real leaf remains outside the fixed
// integral aggregate profile.
module tb;
    typedef struct { real value; logic [7:0] data; } native_t;
    native_t left, right, result;
    function automatic native_t choose(input logic selector, input native_t a, b);
        return selector ? a : b;
    endfunction
    initial result = choose(1'b1, left, right);
endmodule
