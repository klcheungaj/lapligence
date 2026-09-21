// llg-test-fixture: IEEE 1800-2009 §§7.6, 23.2.2, and 23.3.3. A fixed-array
// input value with an incompatible packed element width remains rejected.
typedef logic [7:0] lane_t;
typedef logic [15:0] wide_lane_t;
typedef lane_t row_t [0:1];
typedef wide_lane_t wide_row_t [0:1];

module child (
    input row_t a,
    output lane_t y
);
    assign y = a[0];
endmodule

module tb;
    wide_row_t wide;
    lane_t y;

    child c(.a(wide), .y(y));

    initial begin
        wide[0] = 16'h1010;
        wide[1] = 16'h2020;
        #1;
        $display("unexpected=%h", y);
        $finish(0);
    end
endmodule
