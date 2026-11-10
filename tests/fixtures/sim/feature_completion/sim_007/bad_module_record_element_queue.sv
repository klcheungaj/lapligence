// SIM-007 boundary: each element of a member array of a module record owns its
// own queue, so a queue method on the element chosen by a run-time index has
// no single container to name. It rejects with its location; a constant index
// (`m.e[1].q`) and a fixed array of records (`a[k].q`) work.
module tb;
    typedef struct { int q[$]; int e2; } el_t;
    typedef struct { el_t e[3]; string n; } mr_t;
    mr_t m;
    int k = 1;
    initial begin
        m.e[k].q.push_back(1);
        $finish(0);
    end
endmodule
