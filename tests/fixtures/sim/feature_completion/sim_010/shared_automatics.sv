// SIM-010: join_none branches share the enclosing automatic variables with
// the declaring activation (SV 6.21, 9.3.2): each side sees the other's
// writes, a branch keeps the variable alive after the task returns, sibling
// branches update one variable, and a for-loop variable read by the
// branches is the one variable, read after the loop finished.
module tb;
    task automatic t();
        int x = 1;
        fork
            begin
                #2 $display("child sees %0d", x);
                x = 9;
            end
        join_none
        x = 7;
        #3 $display("parent sees %0d", x);
    endtask
    task automatic outlive(int n);
        int y = n;
        fork
            begin
                #3 $display("late %0d", y);
            end
        join_none
        y = n * 10;
    endtask
    task automatic loop();
        for (int i = 0; i < 3; i++)
            fork
                #1 $display("i %0d", i);
            join_none
        #2;
    endtask
    initial begin
        t();
        outlive(4);
        outlive(6);
        #5;
        begin
            automatic int z = 0;
            fork
                begin #1 z++; end
                begin #2 z++; end
            join_none
            #3 $display("z %0d", z);
        end
        loop();
    end
endmodule
