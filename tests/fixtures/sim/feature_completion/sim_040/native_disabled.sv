// SIM-040: an import that returns because a disable cancelled it copies
// nothing back (SV 35.9, decision S41-D5), whatever its formals are: a
// structure with a string, a dynamic array, a string array and a real array
// keep the values they had before the call. Build native_disabled.c into a
// shared library and pass it with --dpi-lib.
module tb;
    typedef struct { real r; string s; } rec_t;
    import "DPI-C" context task nd_task(output rec_t o, inout int a [], output string s [2],
        output real q [2]);
    import "DPI-C" context function void nd_func(output rec_t o, inout int a [],
        output string s [2], output real q [2]);
    export "DPI-C" task nd_kill_task;
    export "DPI-C" function nd_kill_func;

    rec_t o;
    int a [];
    string s [2];
    real q [2];

    task nd_kill_task();
        disable blk_t;
    endtask

    function void nd_kill_func();
        disable blk_f;
    endfunction

    task automatic show(input string tag);
        $display("%s o=%g %s a=%p s=%s %s q=%g %g", tag, o.r, o.s, a, s[0], s[1], q[0], q[1]);
    endtask

    initial begin
        o.r = 1.5; o.s = "keep";
        a = new[2];
        a[0] = 7; a[1] = 8;
        s[0] = "s0"; s[1] = "s1";
        q[0] = 0.25; q[1] = 0.5;
        begin : blk_t
            nd_task(o, a, s, q);
        end
        show("task");
        begin : blk_f
            nd_func(o, a, s, q);
        end
        show("func");
        $finish;
    end
endmodule
