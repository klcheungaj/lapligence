// A tagged union with a string member nested in a record has no tag check
// for member accesses below the record, so its storage is rejected.
typedef union tagged { void None; int I; string S; } value_t;
typedef struct { value_t u; int k; } holder_t;

module tb;
    holder_t r;

    initial begin
        r.u = tagged S "x";
        $display("%s", r.u.S);
    end
endmodule
