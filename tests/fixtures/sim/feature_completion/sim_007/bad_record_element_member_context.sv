// SIM-007 boundary: a queue member of a queue's record element is staged
// around assignment, call and system-task statements only; an `if`
// condition (or a loop header) that names it is rejected explicitly.
module tb;
    typedef struct { string s; int q[$]; } rec_t;
    rec_t r;
    rec_t list[$];
    initial begin
        r.q = '{1, 2};
        list.push_back(r);
        if (list[0].q.size() > 1) $display("two");
        $finish(0);
    end
endmodule
