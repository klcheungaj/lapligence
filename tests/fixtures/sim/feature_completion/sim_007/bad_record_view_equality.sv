// SIM-007 boundary: a nested record with an associative-array member has
// no supported equality (associative-array equality is not supported), as
// for whole records.
typedef struct {
    string n;
    int k[string];
} in_t;

typedef struct {
    in_t sub;
    int z;
} r_t;

module tb;
    r_t x, y;
    initial begin
        $display("%0d", x.sub == y.sub);
        $finish(0);
    end
endmodule
