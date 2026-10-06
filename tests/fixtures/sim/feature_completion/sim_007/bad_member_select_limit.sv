// SIM-007 boundary: a run-time index selects one of at most 64 member-array
// elements of a native record; a 65-element member array is rejected at
// code generation, while 64 elements are accepted (see native_member_select).
module tb;
    typedef struct { string s[65]; } wide_t;
    wide_t w;
    int k;
    initial begin
        k = 3;
        w.s[k] = "x";
        $finish(0);
    end
endmodule
