// SIM-007 boundary: a queue whose elements are records with an associative
// member is legal (SV 7.2, 7.8, 7.10) but rejected at code generation: an
// element keeps queue and dynamic-array members as nested dynamic arrays
// inside its value, and the runtime has no nested associative form.
module tb;
    typedef struct { string s; int a[string]; } rec_t;
    rec_t r;
    rec_t list[$];
    initial begin
        r.a["k"] = 1;
        list.push_back(r);
        $display("%0d", list.size());
        $finish(0);
    end
endmodule
