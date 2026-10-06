// SIM-011 boundary: a class property of a record type with a queue member is
// legal (SV 7.2, 8.4) but is rejected with a dedicated diagnostic: per-object
// record values carry no companion containers yet.
typedef struct {
    string s;
    int q[$];
} rq_t;

class holder_c;
    rq_t m;
endclass

module tb;
    holder_c h;

    initial begin
        h = new;
        h.m.q.push_back(1);
        $display("%0d", h.m.q.size());
    end
endmodule
