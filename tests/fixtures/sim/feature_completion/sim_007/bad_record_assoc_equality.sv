// SIM-007 boundary: associative arrays have no equality operator support in
// llg yet, so equality of records with an associative member is rejected
// like equality of two associative variables.
module tb;
    typedef struct { int k[string]; } rec_t;
    rec_t m, n;
    initial begin
        m.k["a"] = 1;
        $display("%0d", m == n);
        $finish(0);
    end
endmodule
