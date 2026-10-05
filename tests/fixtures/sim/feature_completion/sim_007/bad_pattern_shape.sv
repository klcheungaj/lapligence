// SIM-007 negative: a positional pattern needs one item per element and each
// item must be assignment compatible with its string member (SV 10.9).
module tb;
    typedef struct { int i; string s; } rec_t;
    rec_t m;
    string a[2];
    initial begin
        a = '{"a", "b", "c"};
        m = '{1, 2.5};
        $display("%s %s", a[0], m.s);
    end
endmodule
