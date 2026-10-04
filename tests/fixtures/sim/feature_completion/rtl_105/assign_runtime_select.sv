// RTL-105: continuous assignments to variables through runtime selects
// (IEEE 1800-2009 10.3, A.8.5 variable_lvalue select, 11.5.1, 6.5 with the
// 11.5.3 longest static prefix). A selector change retargets the write; the
// previously selected element keeps its value. An unknown or out-of-range
// selector writes nothing (7.4.6, 11.5.1).
`timescale 1ns/1ns
typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pair_t;
module tb;
    logic [7:0] a [0:3];
    logic [7:0] x;
    integer i;
    logic [7:0] v;
    logic [2:0] j;
    logic y;
    logic [15:0] w;
    int k;
    logic [3:0] z;
    pair_t s [0:1];
    logic t;
    logic [7:0] m [0:1];
    logic [7:0] n;
    logic u;
    logic [15:0] cat_in;

    assign a[i] = x;
    assign v[j] = y;
    assign w[k +: 4] = z;
    assign s[t].lo = x[3:0];
    assign {m[u], n} = cat_in;

    task show;
        $display("%h %h %h %h | %b | %h | %h %h | %h %h %h",
                 a[0], a[1], a[2], a[3], v, w, s[0], s[1], m[0], m[1], n);
    endtask

    initial begin
        i = 1; x = 8'h11; j = 2; y = 1; k = 4; z = 4'ha; t = 0; u = 0;
        cat_in = 16'h1234;
        #1 show();
        // Selector changes alone retarget every assignment.
        i = 2; j = 5; k = 8; t = 1; u = 1;
        #1 show();
        // Value changes write the current targets.
        x = 8'h22; y = 0; z = 4'h5; cat_in = 16'h5678;
        #1 show();
        // An unknown element selector writes nothing; `s[t].lo` still follows x.
        i = 'x; x = 8'h33;
        #1 show();
        // An out-of-range selector writes nothing.
        i = 7;
        #1 show();
        i = 0;
        #1 show();
        $finish(0);
    end
endmodule
