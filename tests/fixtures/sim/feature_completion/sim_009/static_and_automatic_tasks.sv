// SIM-009: two concurrent calls of a static task share its static local, so
// the later write wins for both; two concurrent calls of an automatic task
// keep independent values across the suspension (SV 6.21, 13.3.1).
module tb;
    task t(input int n);
        int keep;
        keep = n;
        #2 $display("static %0d", keep);
    endtask
    task automatic a(input int n);
        int keep;
        keep = n;
        #2 $display("auto %0d", keep);
    endtask
    initial begin
        fork
            t(1);
            t(2);
            a(3);
            a(4);
        join
    end
endmodule
