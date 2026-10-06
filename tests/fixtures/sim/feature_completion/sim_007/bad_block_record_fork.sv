// SIM-007 boundary: each loop iteration enters the block again while the
// `join_none` branch of the previous entry still reads its automatic record
// (SV 6.21, 9.3.2). One storage per declaration cannot keep both
// activations, so the record is rejected rather than shared.
module tb;
    typedef struct { string s; int n; } r_t;
    initial begin
        for (int i = 0; i < 2; i++) begin
            automatic r_t r;
            r.n = i;
            fork
                #1 $display("%0d", r.n);
            join_none
        end
        #5 $finish(0);
    end
endmodule
