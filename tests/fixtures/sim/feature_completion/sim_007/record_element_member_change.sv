// A condition that changes a queue member of a record element (`pop_back`)
// changes the element: the member is copied out and written back.
module tb;
    typedef struct { string s; int q[$]; } rec_t;
    rec_t r;
    rec_t list[$];
    initial begin
        r.q = '{1, 2};
        list.push_back(r);
        if (list[0].q.pop_back() > 1) $display("two");
        $display("%0d", list[0].q.size());
        $finish(0);
    end
endmodule
