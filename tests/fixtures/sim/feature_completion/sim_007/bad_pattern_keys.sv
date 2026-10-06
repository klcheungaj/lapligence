// SIM-007 negative: a pattern may name each array index and each member at
// most once (SV 10.9.1, 10.9.2); the frontend rejects both duplicates.
module tb;
    typedef struct { int i; string s; } rec_t;
    rec_t m;
    string a[2];
    initial begin
        a = '{0: "a", 1: "b", 0: "c"};
        m = '{s: "x", s: "y", i: 1};
        $display("%s %s", a[0], m.s);
    end
endmodule
