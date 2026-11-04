// An event-controlled nonblocking write to a member of a record element is
// legal (SV 10.4.2) but rejected: the detached event assignment does not carry
// the element position captured at issue.
module tb;
    typedef struct { string s; int n; } rec_t;
    rec_t fixed[2];
    event e;
    initial begin
        fixed[1].n <= @(e) 9;
        #1 -> e;
        #1 $display("%0d", fixed[1].n);
        $finish(0);
    end
endmodule
