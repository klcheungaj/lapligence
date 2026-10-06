// SIM-007 boundary: a run-time index selects one of at most 64 member-array
// elements of a native record; a 64-element member array is accepted at
// code generation; with 65 elements it is rejected (bad_member_select_limit).
module tb;
    typedef struct { string s[64]; } wide_t;
    wide_t w;
    int k;
    initial begin
        k = 3;
        w.s[k] = "x"; k = 63; w.s[k] = "y"; $display("%s%s [%s]", w.s[3], w.s[63], w.s[2]);
        $finish(0);
    end
endmodule
