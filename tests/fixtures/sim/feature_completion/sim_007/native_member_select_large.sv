// SIM-007: a native record member array above the 4,096-element dense
// threshold lives in a container, so a run-time select does not expand it.
module tb;
    typedef struct { string s[4097]; } wide_t;
    wide_t w;
    int k;
    initial begin
        k = 4096;
        w.s[k] = "x";
        $display("%s", w.s[k]);
        $finish(0);
    end
endmodule
