// SIM-009: event expressions reading a task's by-value formals take the
// typed call path: the evaluator copies the formals when the control arms,
// so a task with a native record formal and a recursive task can use them
// (SV 9.4.2, 13.3.1).
module tb;
    logic clk = 0;
    always #1 clk = ~clk;
    int addr;
    typedef struct { int a; string s; } r_t;
    task automatic wait_addr(input int mine, input r_t r);
        @(posedge clk iff (addr == mine)) $display("%s hit %0d at %0d", r.s, mine, $time);
    endtask
    task automatic chain(input int n);
        @(posedge clk iff (addr >= n)) $display("chain %0d at %0d", n, $time);
        if (n > 0) chain(n - 1);
    endtask
    initial begin
        r_t p;
        p.s = "dev";
        fork
            wait_addr(3, p);
            chain(2);
        join_none
        #4 addr = 2;
        #6 addr = 3;
        #4 $finish;
    end
endmodule
