// An event-controlled nonblocking write of a record with a queue member is
// legal (SV 9.4.5, 10.4.2) but rejected: the detached process that waits for
// the control has no frame storage for the queue copied at issue.
module tb;
    typedef struct { string s; int q[$]; } rec_t;
    rec_t a, b;
    event e;
    initial begin
        a <= @(e) b;
        #1 -> e;
        #1 $display("%s", a.s);
        $finish(0);
    end
endmodule
