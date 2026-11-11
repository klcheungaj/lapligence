// SIM-040: a context imported task whose foreign code waits in an exported
// task keeps its copies of a dynamic array, a structure with a string, a
// string array and a real open array across the wait; what the exported task
// writes to the actuals meanwhile is replaced when the import returns and
// its inouts are copied out (SV 35.6.1). Build native_context_task.c into a
// shared library and pass it with --dpi-lib.
module tb;
    typedef struct { real r; string s; } rec_t;
    import "DPI-C" context task ct_wait(inout int a [], inout rec_t r, inout string names [2],
        input real w []);
    export "DPI-C" task ct_tick;

    int d [];
    rec_t r;
    string names [2];
    real w [3];

    task ct_tick(input int delay);
        #delay;
        d = new[5](d);
        r.s = "export";
        $display("tick at %0d d=%p r.s=%s", $time, d, r.s);
    endtask

    initial begin
        d = new[2];
        d[0] = 1; d[1] = 2;
        r.r = 1.5; r.s = "start";
        names[0] = "a"; names[1] = "b";
        w[0] = 1.0; w[1] = 2.0; w[2] = 3.0;
        ct_wait(d, r, names, w);
        $display("after at %0d d=%p r=%g %s names=%s %s", $time, d, r.r, r.s, names[0],
            names[1]);
        $finish;
    end

    initial #3 $display("other process at %0d d=%p", $time, d);
endmodule
