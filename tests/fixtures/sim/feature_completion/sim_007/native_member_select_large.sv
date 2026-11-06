// SIM-007 boundary: a native record member array above the 4,096-element dense
// threshold is rejected at code generation, so no run-time select expands it.
module tb;
    typedef struct { string s[4097]; } wide_t;
    wide_t w;
    int k;
    initial begin
        k = 3;
        w.s[k] = "x";
        $finish(0);
    end
endmodule
