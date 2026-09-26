// The outer method visits rows, not their unvisited second unpacked dimension.
module tb;
    int matrix [1:0][-2:-1];
    int result;
    initial begin
        matrix[1][-2] = 1; matrix[1][-1] = 2;
        matrix[0][-2] = 3; matrix[0][-1] = 4;
        result = matrix.sum(row) with (row.index(2));
        $display("result=%0d", result);
        $finish(0);
    end
endmodule
