// SIM-007 boundary: a queue member of a queue's record element is read from
// a copy in a condition or loop header, but an `if` condition that changes
// it (`pop_back`) would change only the copy, so it is rejected explicitly.
module tb;
    typedef struct { string s; int q[$]; } rec_t;
    rec_t r;
    rec_t list[$];
    initial begin
        r.q = '{1, 2};
        list.push_back(r);
        if (list[0].q.pop_back() > 1) $display("two");
        $finish(0);
    end
endmodule
