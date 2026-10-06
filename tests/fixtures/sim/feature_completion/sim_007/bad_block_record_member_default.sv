// SIM-007 boundary: member defaults of a record with string members
// (SV 7.2.2) are not applied to procedural-block storage, so the record is
// rejected rather than left at the type's uninitialized value.
module tb;
    typedef struct { string s = "d"; int n; } r_t;
    initial begin
        r_t r;
        r.n = 1;
        $display("%s %0d", r.s, r.n);
        $finish(0);
    end
endmodule
