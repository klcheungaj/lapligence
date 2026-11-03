// SIM-007 boundary: a nonblocking write of a packed member of a fixed array's
// native record element is legal (SV 10.4.2) but rejected: the write would go
// through an access into the element's value, which a whole-element write may
// replace before the NBA region.
module tb;
    typedef struct { string s; int n; } rec_t;
    rec_t fixed[2];
    initial begin
        fixed[1].n <= 9;
        #1 $display("%0d", fixed[1].n);
        $finish(0);
    end
endmodule
