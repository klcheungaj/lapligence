// SIM-007 boundary: a subroutine formal whose record type has a queue member
// is legal (SV 7.2, 13.5) but rejected at code generation, naming the formal.
module tb;
    typedef struct { string s; int q[$]; } rec_t;
    function automatic int count(rec_t x);
        x.q.push_back(4);
        return x.q.size();
    endfunction
    initial begin
        rec_t t;
        t.s = "a";
        $display("%0d", count(t));
        $finish(0);
    end
endmodule
