// An event-controlled nonblocking write of a record with a queue member keeps
// the queue of the issue until the control (SV 9.4.5, 10.4.2).
module tb;
    typedef struct { string s; int q[$]; } rec_t;
    rec_t a, b;
    event e;
    initial begin
        b.s = "b";
        b.q = {1, 2};
        a <= @(e) b;
        b.s = "later";
        b.q.delete();
        #1 $display("%0t [%s] %0d", $time, a.s, a.q.size());
        -> e;
        #1 $display("%0t [%s] %0d", $time, a.s, a.q.size());
        $finish(0);
    end
endmodule
