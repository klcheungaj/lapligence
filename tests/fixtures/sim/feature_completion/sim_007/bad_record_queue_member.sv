// SIM-007 boundary: a record member that is a queue is legal (SV 7.2, 7.10)
// but has no record storage yet; the variable is rejected at code generation.
module tb;
    typedef struct { string s; int q[$]; } rec_t;
    rec_t m;
    initial begin
        m.q.push_back(3);
        $display("%0d", m.q[0]);
        $finish(0);
    end
endmodule
