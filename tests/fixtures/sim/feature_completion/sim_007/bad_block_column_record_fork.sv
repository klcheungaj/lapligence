// SIM-007 boundary: an automatic record with a member array of at least 16
// cells is lexical columns of its activation, which have no fork frame form;
// a fork branch that names it is rejected (SV 6.21, 9.3.2).
module tb;
    typedef struct { string s; int a[64]; } r_t;
    initial begin
        begin
            automatic r_t r;
            r.a[1] = 2;
            fork
                #1 $display("%0d", r.a[1]);
            join_none
        end
        #5 $finish(0);
    end
endmodule
