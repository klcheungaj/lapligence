// RTL-105: runtime-selected continuous assignments to variables composed with
// descriptor-backed storage, delays, multidimensional rows, generate loops,
// expression selectors and a selector that is written by another continuous
// assignment (IEEE 1800-2009 10.3, 10.3.3, 6.5, 7.4.6, 11.5.1, 11.5.3).
`timescale 1ns/1ns
module sel_cell #(parameter int N = 2) (input logic [7:0] d, input int sel,
                                    output logic [7:0] q [0:N-1]);
    assign q[sel] = d;
endmodule
module tb;
    // 65,537 cells: stored behind a descriptor, never expanded per cell.
    logic [7:0] big [0:65536];
    int bi;
    logic [7:0] bx;
    assign big[bi] = bx;

    // Delayed (inertial) driver; each update completes before the selector
    // changes.
    logic [7:0] dl [0:3];
    int di;
    logic [7:0] dx;
    assign #2 dl[di] = dx;

    // Two-dimensional array with both selectors runtime.
    logic [3:0] grid [0:2][0:1];
    int gr, gc;
    logic [3:0] gx;
    assign grid[gr][gc] = gx;

    // Expression selector into a wide packed variable.
    logic [127:0] wide;
    logic [3:0] wk;
    logic [7:0] wx;
    assign wide[wk * 8 +: 8] = wx;

    // The selector is itself driven by a continuous assignment.
    logic [7:0] chain [0:3];
    logic [1:0] base;
    logic [1:0] ci;
    logic [7:0] cv;
    assign ci = base + 2'd1;
    assign chain[ci] = cv;

    // Inside a child whose output array is linked to the parent.
    logic [7:0] gd [0:1];
    int gs [0:1];
    logic [7:0] gq0 [0:1];
    logic [7:0] gq1 [0:1];
    sel_cell #(.N(2)) c0(.d(gd[0]), .sel(gs[0]), .q(gq0));
    sel_cell #(.N(2)) c1(.d(gd[1]), .sel(gs[1]), .q(gq1));

    // Generated drivers: the longest static prefixes rows[0] and rows[1]
    // are disjoint, so each row has one continuous writer.
    logic [7:0] rows [0:1][0:2];
    int rs [0:1];
    for (genvar g = 0; g < 2; g++) begin : gen
        localparam logic [7:0] OFFSET = g;
        assign rows[g][rs[g]] = gd[g] + OFFSET;
    end

    // A procedural writer of another row is not in the prefix mix[0].
    logic [7:0] mix [0:1][0:1];
    int mi;
    assign mix[0][mi] = gd[0];
    initial mix[1][1] = 8'h77;

    initial begin
        mi = 1;
        bi = 65536; bx = 8'ha1;
        di = 1; dx = 8'h11;
        gr = 2; gc = 1; gx = 4'h7;
        wk = 15; wx = 8'hc3;
        base = 2'd0; cv = 8'h5a;
        gd[0] = 8'h40; gs[0] = 1; gd[1] = 8'h50; gs[1] = 0;
        rs[0] = 2; rs[1] = 0;
        #3 $display("%h %h | %h %h | %h %h | %h %h | %h %h %h %h | %h %h %h %h | %h %h %h %h %h %h",
                    big[0], big[65536], dl[1], dl[2], grid[2][1], grid[2][0],
                    wide[127:120], wide[7:0], chain[0], chain[1], chain[2], chain[3],
                    gq0[0], gq0[1], gq1[0], gq1[1],
                    rows[0][0], rows[0][1], rows[0][2], rows[1][0], rows[1][1], rows[1][2]);
        bi = 0;
        di = 2;
        gc = 0;
        wk = 0;
        base = 2'd2;
        gs[0] = 0; gs[1] = 1;
        rs[0] = 0; rs[1] = 3;
        mi = 0;
        #3 $display("%h %h | %h %h | %h %h | %h %h | %h %h %h %h | %h %h %h %h | %h %h %h %h %h %h",
                    big[0], big[65536], dl[1], dl[2], grid[2][1], grid[2][0],
                    wide[127:120], wide[7:0], chain[0], chain[1], chain[2], chain[3],
                    gq0[0], gq0[1], gq1[0], gq1[1],
                    rows[0][0], rows[0][1], rows[0][2], rows[1][0], rows[1][1], rows[1][2]);
        $display("%h %h %h %h", mix[0][0], mix[0][1], mix[1][0], mix[1][1]);
        $finish(0);
    end
endmodule
