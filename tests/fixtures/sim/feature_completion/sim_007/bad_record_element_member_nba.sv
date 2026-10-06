// SIM-007 boundary: a nonblocking write of a whole queue member of a fixed
// array's record element is legal (SV 10.4.2) but rejected: the member is a
// nested container inside the element value, which no queued update owns.
module tb;
    typedef struct { string s; int q[$]; } rec_t;
    rec_t r;
    rec_t fixed[2];
    initial begin
        r.q = '{1, 2};
        fixed[0].q <= r.q;
        #1 $display("%0d", fixed[0].q.size());
        $finish(0);
    end
endmodule
