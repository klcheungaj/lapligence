// SIM-011 A02: class methods are automatic (SV 8.6, 13.3.1): concurrent and
// recursive activations of one method keep independent locals, while a
// static property is shared by all of them.
class W;
    int id;
    static int calls;
    function new(int i);
        id = i;
    endfunction
    task work(int delay, int v, output int res);
        int acc;
        calls++;
        acc = v;
        #(delay);
        acc = acc + id;
        res = acc;
    endtask
    function int fact(int n);
        if (n <= 1) return 1;
        return n * fact(n - 1);
    endfunction
    task count_down(int n, output int sum);
        int local_n;
        int sub;
        local_n = n;
        if (n == 0) begin
            sum = 0;
            return;
        end
        #1 count_down(n - 1, sub);
        sum = sub + local_n;
        $display("level %0d t=%0d sum=%0d", local_n, $time, sum);
    endtask
endclass

module tb;
    W a;
    W b;
    int r1;
    int r2;
    int r3;
    int s;

    initial begin
        a = new(100);
        b = new(200);
        fork
            a.work(3, 1, r1);
            a.work(1, 2, r2);
            b.work(2, 3, r3);
        join
        $display("t=%0d r1=%0d r2=%0d r3=%0d calls=%0d", $time, r1, r2, r3, W::calls);
        $display("fact=%0d", a.fact(5));
        b.count_down(3, s);
        $display("t=%0d sum=%0d", $time, s);
        $finish;
    end
endmodule
