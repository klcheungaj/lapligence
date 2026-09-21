// llg-test-fixture: IEEE 1800-2009 §§7.6, 23.2.2, and 23.3.3. A fixed-array
// input value with an incompatible unpacked rank remains rejected.
typedef logic [7:0] lane_t;
typedef lane_t row_t [0:1];
typedef lane_t matrix_t [0:1][0:1];

module child (
    input row_t a,
    output lane_t y
);
    assign y = a[0];
endmodule

module tb;
    matrix_t matrix;
    lane_t y;

    child c(.a(matrix), .y(y));

    initial begin
        matrix[0][0] = 8'h10;
        matrix[0][1] = 8'h20;
        matrix[1][0] = 8'h30;
        matrix[1][1] = 8'h40;
        #1;
        $display("unexpected=%h", y);
        $finish(0);
    end
endmodule
