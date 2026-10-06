// SIM-008: output and inout actuals naming one element of a queue, dynamic
// array or associative array (SV 13.5.1-13.5.2). Keys are evaluated once at
// the call; the formal's final value is stored after the callee returns.
module tb;
    int q[$];
    int aa[string];
    int ai[int];
    real rq[$];
    int d[];
    int k;

    task automatic outq(output int x);
        x = 7;
    endtask

    task automatic incq(inout int x);
        x = x + 10;
    endtask

    task automatic outr(output real x);
        x = 2.5;
    endtask

    task automatic later(output int x);
        #1;
        x = 3;
    endtask

    initial begin
        q = '{1, 2};
        outq(q[1]);
        $display("q %0d %0d", q[0], q[1]);
        incq(q[0]);
        $display("inc %0d", q[0]);
        aa["y"] = 0;
        outq(aa["y"]);
        outq(aa["z"]);
        $display("aa %0d %0d %0d", aa["y"], aa["z"], aa.num());
        ai[5] = 1;
        incq(ai[5]);
        $display("ai %0d", ai[5]);
        rq = '{0.5};
        outr(rq[0]);
        $display("rq %.1f", rq[0]);
        d = new[2];
        k = 1;
        outq(d[k]);
        $display("d %0d %0d", d[0], d[1]);
        k = 0;
        fork
            later(q[k]);
        join_none
        #0 k = 1;
        q.push_front(9);
        #2 $display("later %0d %0d %0d", q[0], q[1], q[2]);
        $finish(0);
    end
endmodule
