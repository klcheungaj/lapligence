// SIM-007 boundary: a statement stages a queue member of a container record
// element as a copy, which a `ref` formal must not alias (SV 13.5.2), so the
// member as a ref actual is rejected; inout actuals copy back.
module tb;
    typedef struct { string s; int q[$]; } rec_t;
    rec_t r;
    rec_t list[$];
    task automatic grow(ref int q[$]);
        q.push_back(1);
    endtask
    initial begin
        list.push_back(r);
        grow(list[0].q);
        $finish(0);
    end
endmodule
