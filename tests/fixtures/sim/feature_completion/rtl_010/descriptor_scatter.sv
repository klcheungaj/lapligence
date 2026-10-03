// IEEE 1800-2009 7.6, 10.3 and 10.9: continuous pattern drivers with
// 65,537-cell rows scatter whole descriptor rows. Sources are a row of a
// two-dimensional array, typed patterns with array and default items, and a
// leaf that also reads another leaf of the same driver.
module tb;
    typedef logic [7:0] row_t[0:65536];
    typedef row_t pair_t[0:1];
    row_t d0, d1, r0, r1, f0, f1, src;
    row_t two[0:1];
    logic [7:0] fill = 8'h21;
    assign '{d0, d1} = two;
    assign '{r0, r1} = pair_t'{src, '{default: fill}};
    assign '{f0, f1} = pair_t'{two[0], f0};
    initial begin
        two[0][65536] = 8'h03;
        two[1][0] = 8'h04;
        src[0] = 8'h05;
        #1 $display("%h %h %h %h | %h %h %h | %h %h", d0[65536], d0[0], d1[0], d1[65536],
                    r0[0], r1[0], r1[65536], f0[65536], f1[65536]);
        two[0][65536] = 8'h44;
        two[1][0] = 8'h55;
        fill = 8'h66;
        src[0] = 8'h77;
        #1 $display("%h %h %h %h | %h %h %h | %h %h", d0[65536], d0[0], d1[0], d1[65536],
                    r0[0], r1[0], r1[65536], f0[65536], f1[65536]);
        $finish(0);
    end
endmodule
