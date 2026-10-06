// SIM-007 boundary: a queue whose elements are records with a queue member is
// legal (SV 7.2, 7.10) but rejected at code generation: container elements
// hold nested containers inside the element value, which record members and
// subroutine values do not share.
module tb;
    typedef struct { string s; int q[$]; } rec_t;
    rec_t r;
    rec_t list[$];
    initial begin
        r.q = '{1};
        list.push_back(r);
        $display("%0d", list.size());
        $finish(0);
    end
endmodule
