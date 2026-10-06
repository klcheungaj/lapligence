// SIM-007 boundary: a nonblocking assignment of a whole record with a queue
// member would need an issue-time copy of the member container; it is
// rejected at code generation rather than written as a blocking copy.
module tb;
    typedef struct { string s; int q[$]; } rec_t;
    rec_t m, n;
    initial begin
        m.q.push_back(1);
        n <= m;
        #1 $display("%0d", n.q.size());
        $finish(0);
    end
endmodule
