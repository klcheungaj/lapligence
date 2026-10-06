// `void'(q.pop_front())` / `void'(q.pop_back())` remove an element and
// discard it (SV 6.24.1, 7.10.2.6-7) for packed, real and string queues and
// for a queue member of a record.
module tb;
    typedef struct { string s; int q[$]; } rec_t;
    int iq[$];
    real rq[$];
    string sq[$];
    rec_t m;
    initial begin
        iq = '{1, 2, 3};
        void'(iq.pop_front());
        void'(iq.pop_back());
        rq = '{0.5, 1.5};
        void'(rq.pop_back());
        sq = '{"a", "b"};
        void'(sq.pop_front());
        m.q = '{7, 8};
        void'(m.q.pop_front());
        $display("%0d %0d %0.1f %0d %s %0d %0d", iq.size(), iq[0], rq[0], sq.size(), sq[0], m.q.size(), m.q[0]);
        $finish(0);
    end
endmodule
