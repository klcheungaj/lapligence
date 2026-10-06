// SIM-011: a timed override calls its base implementation through `super`
// and a module task through a hierarchical name; a static task suspends
// (SV 8.15, 8.10, 23.6).
module tb;
    int log_v;
    task automatic note(int v);
        #1 log_v = log_v + v;
    endtask

    class Base;
        int x;
        virtual task step();
            #1 x = x + 1;
        endtask
        static task s_wait(int n);
            #(n);
        endtask
    endclass

    class Der extends Base;
        virtual task step();
            super.step();
            #2 x = x + 10;
            tb.note(x);
        endtask
    endclass

    Der d;
    Base b;

    initial begin
        d = new;
        b = d;
        b.step();
        $display("t=%0d x=%0d log=%0d", $time, d.x, log_v);
        Base::s_wait(3);
        $display("t=%0d", $time);
        $finish;
    end
endmodule
