// SV 23.3.3.2, 6.5, 7.4.6: runtime-selected output targets of record members
// and array elements; an unknown selector writes nothing.
typedef struct { logic [3:0] lo; logic [3:0] hi; } pair_t;

module child(output logic [3:0] a, output logic [7:0] b);
    initial begin
        a = 4'h3;
        b = 8'h5a;
        #2;
        a = 4'h9;
        b = 8'ha5;
    end
endmodule

module tb;
    pair_t s[3];
    logic [7:0] m[0:3];
    int i = 2;
    logic [1:0] k = 2'bx1;
    child c(.a(s[i].lo), .b(m[k]));
    initial begin
        #1 $display("%h %h %h | %h %h %h %h", s[0].lo, s[1].lo, s[2].lo, m[0], m[1], m[2], m[3]);
        i = 0;
        k = 2'd3;
        #2 $display("%h %h %h | %h %h %h %h", s[0].lo, s[1].lo, s[2].lo, m[0], m[1], m[2], m[3]);
        $finish(0);
    end
endmodule
