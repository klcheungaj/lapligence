// SIM-009: a recursive timed task with string, record and queue locals, an
// input event formal and a ref alias: every activation keeps its own locals
// across suspension and recursion (SV 13.3.1, 13.5.2, 15.5).
module tb;
    event go;
    int total;
    typedef struct { int a; string s; } r_t;
    task automatic rec(int n, input event e, ref int acc, input r_t r);
        string tag;
        r_t mine;
        int q[$];
        tag = $sformatf("%s%0d", r.s, n);
        mine.a = n;
        mine.s = tag;
        q.push_back(n * 10);
        @e;
        acc += mine.a;
        if (n > 0) rec(n - 1, e, acc, mine);
        $display("%s %0d %0d %0d", tag, mine.a, q[0], $time);
    endtask
    initial begin
        r_t seed;
        seed.s = "L";
        fork rec(2, go, total, seed); join_none
        repeat (3) #1 ->go;
        #1 $display("total %0d", total);
    end
endmodule
