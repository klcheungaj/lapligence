// llg-test-fixture: tests/fixtures/sim/syn016_elaboration/nominal_enum_mismatch.sv
// Identical storage widths and values do not make distinct enums assignment compatible.
module tb;
    typedef enum logic [1:0] {A_ZERO = 0, A_ONE = 1} first_t;
    typedef enum logic [1:0] {B_ZERO = 0, B_ONE = 1} second_t;
    first_t first;
    second_t second;
    initial begin
        first = A_ONE;
        second = first;
    end
endmodule
