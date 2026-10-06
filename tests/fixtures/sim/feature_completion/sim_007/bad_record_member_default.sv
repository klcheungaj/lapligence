// SIM-007 boundary: a member default whose value is a record with a string
// member has no captured constant form, so the record is rejected rather
// than left at its uninitialized value (SV 7.2.2).
typedef struct {
    string n;
    int k;
} in_t;

typedef struct {
    in_t inner = '{n: "d", k: 1};
    int z;
} r_t;

module tb;
    r_t m;
    initial begin
        $display("%s %0d", m.inner.n, m.inner.k);
        $finish(0);
    end
endmodule
